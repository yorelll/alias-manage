//! Read-only diagnostics shared by the CLI and the GUI.
//!
//! Both front ends call [`diagnose`] so they agree on target shells, the
//! monotonic revision floor, loader probing, and finding codes. The helper
//! never repairs anything; actionable findings are surfaced to the UI.

use crate::{
    config::AppPaths,
    error::AliasError,
    integration,
    model::{AliasRecord, ShellKind},
    shells::common,
    storage::Database,
    sync,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

const KNOWN_SHELLS: [ShellKind; 4] = [
    ShellKind::Bash,
    ShellKind::Zsh,
    ShellKind::PowerShell5,
    ShellKind::PowerShell7,
];

/// One diagnostic finding. `code` is stable and localized by the GUI; messages
/// stay in the backend for the CLI and unknown-code fallbacks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorFinding {
    pub shell: Option<ShellKind>,
    pub severity: &'static str,
    pub code: &'static str,
    pub message: String,
    pub action: Option<String>,
    pub detail: Option<String>,
}

fn finding(
    shell: Option<ShellKind>,
    severity: &'static str,
    code: &'static str,
    message: String,
) -> DoctorFinding {
    DoctorFinding {
        shell,
        severity,
        code,
        message,
        action: None,
        detail: None,
    }
}

fn shell_label(shell: &ShellKind) -> String {
    format!("{shell:?}")
}

fn loader_read_finding(shell: &ShellKind, error: &AliasError) -> DoctorFinding {
    let (code, action, suffix) = match error {
        AliasError::Io(io) => match io.kind() {
            std::io::ErrorKind::PermissionDenied => (
                "loader_permission",
                "检查 Shell 配置文件权限后重试",
                "权限不足",
            ),
            std::io::ErrorKind::InvalidData => (
                "loader_unreadable",
                "检查 Shell 配置文件的编码后重试",
                "编码无效",
            ),
            _ => ("loader_io", "检查 Shell 配置文件后重试", "读取失败"),
        },
        AliasError::UnsafePath => (
            "loader_unsafe",
            "修复 Alias Manager 标记后重试",
            "标记不成对或顺序错误",
        ),
        AliasError::Config(_) => ("config_invalid", "修复 config.toml 后重试", "配置文件无效"),
        _ => ("loader_unreadable", "检查 Shell 配置文件后重试", "无法读取"),
    };
    let mut value = finding(
        Some(shell.clone()),
        "warning",
        code,
        format!("Shell {} 加载块{}", shell_label(shell), suffix),
    );
    value.action = Some(action.to_string());
    value.detail = Some(error.to_string());
    value
}

/// Builds every read-only diagnostic finding for one config root.
///
/// `database_path_exists` must be captured before opening the database, since
/// opening creates the file. All other state is read from `database`/`aliases`.
pub fn diagnose(
    config_dir: &Path,
    database_path_exists: bool,
    database: &Database,
    aliases: &[AliasRecord],
) -> Result<Vec<DoctorFinding>, AliasError> {
    let paths = AppPaths {
        root: config_dir.to_path_buf(),
    };
    let mut findings = Vec::new();
    if !database_path_exists {
        let mut value = finding(None, "error", "database_missing", "数据库文件缺失".into());
        value.action = Some("创建或选择配置目录".into());
        findings.push(value);
    }

    // Shared target-shell and revision rules: a revision-0 record is treated
    // as revision 1 by sync, so Doctor must render with the same value.
    let referenced_shells = sync::referenced_shells(database, aliases)?;
    let current_revision = sync::current_revision(database, aliases)?;

    let mut managed_names = BTreeMap::new();
    for kind in KNOWN_SHELLS.iter() {
        managed_names.insert(kind.clone(), database.managed_name_set(kind)?);
    }

    let coordinator = sync::SyncCoordinator::new(config_dir);
    let mut stale_details: BTreeMap<ShellKind, String> = BTreeMap::new();
    for kind in KNOWN_SHELLS.iter() {
        let path = paths.generated_path(kind.as_str());
        // Never invent "missing" findings for shells no alias targets, but do
        // validate a leftover generated file that actually exists.
        if !referenced_shells.contains(kind) && !path.exists() {
            continue;
        }
        if !path.exists() {
            findings.push(finding(
                Some(kind.clone()),
                "warning",
                "generated_missing",
                format!("生成文件缺失: {}", shell_label(kind)),
            ));
            continue;
        }
        let content = match std::fs::read(&path) {
            Ok(bytes) => match String::from_utf8(bytes) {
                Ok(content) => content,
                Err(error) => {
                    let mut value = finding(
                        Some(kind.clone()),
                        "warning",
                        "generated_unreadable",
                        format!("生成文件编码无效: {}", shell_label(kind)),
                    );
                    value.detail = Some(error.to_string());
                    findings.push(value);
                    continue;
                }
            },
            Err(error) => {
                let mut value = finding(
                    Some(kind.clone()),
                    "warning",
                    "generated_unreadable",
                    format!("生成文件无法读取: {}", shell_label(kind)),
                );
                value.detail = Some(error.to_string());
                findings.push(value);
                continue;
            }
        };
        let expected = content
            .lines()
            .find_map(|line| line.strip_prefix("# file_checksum: "));
        let actual = sync::content_checksum(&content);
        let mut modified = false;
        match expected {
            Some(expected) if sync::verify_file_checksum(&content, expected) => {}
            Some(expected) => {
                modified = true;
                let mut value = finding(
                    Some(kind.clone()),
                    "warning",
                    "generated_modified",
                    format!(
                        "生成文件校验和不匹配（可能被手工修改）: {}",
                        shell_label(kind)
                    ),
                );
                value.detail = Some(format!("expected {expected}, actual {actual}"));
                findings.push(value);
            }
            None => {
                modified = true;
                let mut value = finding(
                    Some(kind.clone()),
                    "warning",
                    "generated_modified",
                    format!("生成文件缺少校验和标记: {}", shell_label(kind)),
                );
                value.detail = Some(format!("expected <missing>, actual {actual}"));
                findings.push(value);
            }
        }
        if modified {
            continue;
        }
        let Some(names) = managed_names.get(kind) else {
            continue;
        };
        let shell_aliases: Vec<AliasRecord> = aliases
            .iter()
            .filter(|alias| alias.shells.contains(kind))
            .cloned()
            .collect();
        match coordinator.render_shell(&shell_aliases, kind, current_revision, names) {
            Ok(body) => {
                let expected_checksum = sync::content_checksum(&body);
                if expected_checksum != actual {
                    stale_details.insert(
                        kind.clone(),
                        format!("expected checksum {expected_checksum}, actual {actual}"),
                    );
                }
            }
            Err(error) => {
                let mut value = finding(
                    Some(kind.clone()),
                    "warning",
                    "render_failed",
                    format!("无法渲染预期内容: {}", shell_label(kind)),
                );
                value.detail = Some(error.to_string());
                findings.push(value);
            }
        }
    }

    let mut loader_shells: BTreeSet<ShellKind> = referenced_shells.iter().cloned().collect();
    for state in database.shell_states()? {
        loader_shells.insert(state.shell.clone());
        if let Some(error) = &state.last_error {
            let mut value = finding(
                Some(state.shell.clone()),
                "error",
                "sync_failed",
                format!("Shell {} 同步失败", shell_label(&state.shell)),
            );
            value.detail = Some(error.clone());
            findings.push(value);
        }
        if state.applied_revision < current_revision {
            stale_details.entry(state.shell.clone()).or_insert_with(|| {
                format!(
                    "applied revision {}, expected {current_revision}",
                    state.applied_revision
                )
            });
        }
    }

    // Probe real profiles. Shells sharing one profile path are grouped: a
    // block matching any expected loader for that path counts as installed for
    // every shell using it, so Doctor cannot enter an install loop.
    match paths.load_config() {
        Err(error) => {
            let mut value = finding(None, "error", "config_invalid", "配置文件无效".into());
            value.action = Some("修复 config.toml 后重试".into());
            value.detail = Some(error.to_string());
            findings.push(value);
        }
        Ok(config) => {
            let mut groups: BTreeMap<PathBuf, Vec<ShellKind>> = BTreeMap::new();
            for shell in &loader_shells {
                match integration::loader_target(&config, &paths, shell.as_str()) {
                    Ok(target) => groups.entry(target.path).or_default().push(shell.clone()),
                    // No profile exists for this shell on this platform.
                    Err(AliasError::ShellNotInstalled) => {}
                    Err(error) => findings.push(loader_read_finding(shell, &error)),
                }
            }
            for (path, shells) in groups {
                let expected = shells
                    .iter()
                    .filter_map(|shell| {
                        integration::expected_loader(&paths, shell.as_str())
                            .ok()
                            .map(|(_, loader)| loader)
                    })
                    .collect::<Vec<_>>();
                let content = match integration::profile_content(&path) {
                    Ok(content) => content,
                    Err(error) => {
                        for shell in &shells {
                            findings.push(loader_read_finding(shell, &error));
                        }
                        continue;
                    }
                };
                match common::loader_position(&content) {
                    Err(error) => {
                        for shell in &shells {
                            findings.push(loader_read_finding(shell, &error));
                        }
                    }
                    Ok(position) => {
                        let matching = expected
                            .iter()
                            .filter(|loader| integration::block_matches(&content, loader))
                            .count();
                        let installed = path.exists()
                            && position == common::LoaderPosition::AtEnd
                            && matching > 0;
                        if !installed {
                            for shell in &shells {
                                let mut value = finding(
                                    Some(shell.clone()),
                                    "warning",
                                    "loader_missing",
                                    format!("Shell {} 加载块未安装", shell_label(shell)),
                                );
                                value.action = Some("重新安装 Shell 集成".into());
                                value.detail = Some(format!("profile: {}", path.display()));
                                findings.push(value);
                            }
                        } else if shells.len() > 1 && matching < expected.len() {
                            // One block can only source one generated file. A
                            // shared profile therefore cannot load every
                            // shell's aliases; surface that instead of calling
                            // all shells healthy (and without an install button
                            // that would just replace the other shell's block).
                            let names = shells
                                .iter()
                                .map(shell_label)
                                .collect::<Vec<_>>()
                                .join(", ");
                            let mut value = finding(
                                None,
                                "warning",
                                "loader_shared_path",
                                format!(
                                    "Shell 配置路径被多个 Shell 共享，只能加载部分生成文件: {names}"
                                ),
                            );
                            value.detail = Some(format!(
                                "profile: {}; matched {matching} of {} expected loaders",
                                path.display(),
                                expected.len()
                            ));
                            findings.push(value);
                        }
                    }
                }
            }
        }
    }

    for (shell, detail) in stale_details {
        let mut value = finding(
            Some(shell.clone()),
            "warning",
            "stale",
            format!("Shell {} 生成文件过期", shell_label(&shell)),
        );
        value.action = Some("执行同步".into());
        value.detail = Some(detail);
        findings.push(value);
    }
    if let Some(error) = database.reliability_error() {
        let mut value = finding(None, "warning", "unreliable_filesystem", error.to_string());
        value.action = Some("改用本地配置目录".into());
        findings.push(value);
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AliasRecord, ShellKind};

    fn temp_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "aliasmgr-doctor-core-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or_default()
        ))
    }

    fn finding_code<'a>(findings: &'a [DoctorFinding], code: &str) -> Option<&'a DoctorFinding> {
        findings.iter().find(|finding| finding.code == code)
    }

    #[test]
    fn shared_profile_that_only_covers_one_shell_is_reported() {
        let root = temp_root("shared");
        std::fs::create_dir_all(&root).unwrap();
        let paths = AppPaths { root: root.clone() };
        let shared = root.join("shared.rc");
        let mut config = crate::config::AppConfig::default();
        config.shells.bash_rc_path = Some(shared.clone());
        config.shells.zsh_rc_path = Some(shared.clone());
        config.save(&paths).unwrap();
        let database = Database::open(root.join("aliases.db")).unwrap();
        database
            .insert_alias(&AliasRecord {
                name: "gs".into(),
                executable: "git".into(),
                shells: vec![ShellKind::Bash, ShellKind::Zsh],
                ..Default::default()
            })
            .unwrap();

        // Install the bash loader; the shared path now sources bash.sh only,
        // so a zsh-only alias would never load.
        integration::install_loader(&paths, "bash").unwrap();
        let aliases = database.list_aliases().unwrap();
        let findings = diagnose(&root, true, &database, &aliases).unwrap();
        assert!(
            finding_code(&findings, "loader_shared_path").is_some(),
            "a shared profile covering only one shell must be reported: {findings:?}"
        );
        assert!(
            finding_code(&findings, "loader_missing").is_none(),
            "the shared block itself is not missing: {findings:?}"
        );
        let shared_finding = finding_code(&findings, "loader_shared_path").unwrap();
        assert_eq!(shared_finding.severity, "warning");
        assert!(
            shared_finding.action.is_none(),
            "no install button for shared paths"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn invalid_config_and_marker_order_are_reported_distinctly() {
        let root = temp_root("config-marker");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("config.toml"), "not = [valid").unwrap();
        let database = Database::open(root.join("aliases.db")).unwrap();
        database
            .insert_alias(&AliasRecord {
                name: "gs".into(),
                executable: "git".into(),
                shells: vec![ShellKind::Bash],
                ..Default::default()
            })
            .unwrap();
        let aliases = database.list_aliases().unwrap();
        let findings = diagnose(&root, true, &database, &aliases).unwrap();
        assert!(
            finding_code(&findings, "config_invalid").is_some(),
            "malformed config.toml must be config_invalid, not a profile error: {findings:?}"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn inverted_markers_are_reported_as_loader_unsafe() {
        let root = temp_root("inverted-markers");
        std::fs::create_dir_all(&root).unwrap();
        let paths = AppPaths { root: root.clone() };
        let profile = root.join("bashrc");
        let mut config = crate::config::AppConfig::default();
        config.shells.bash_rc_path = Some(profile.clone());
        config.save(&paths).unwrap();
        std::fs::write(
            &profile,
            format!(
                "before\n{}\nuser middle\n{}\nafter\n",
                common::END_MARKER,
                common::START_MARKER
            ),
        )
        .unwrap();
        let database = Database::open(root.join("aliases.db")).unwrap();
        database
            .insert_alias(&AliasRecord {
                name: "gs".into(),
                executable: "git".into(),
                shells: vec![ShellKind::Bash],
                ..Default::default()
            })
            .unwrap();
        let aliases = database.list_aliases().unwrap();
        let findings = diagnose(&root, true, &database, &aliases).unwrap();
        assert!(
            finding_code(&findings, "loader_unsafe").is_some(),
            "inverted markers must not be reported as merely missing: {findings:?}"
        );
        assert!(
            finding_code(&findings, "loader_unreadable").is_none()
                && finding_code(&findings, "config_invalid").is_none(),
            "marker errors must have their own headline: {findings:?}"
        );
        let _ = std::fs::remove_dir_all(root);
    }
}

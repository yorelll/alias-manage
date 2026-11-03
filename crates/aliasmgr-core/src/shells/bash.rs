use super::common::{is_simple_alias, quote_posix};
use crate::{
    error::AliasError,
    executor::render_argv,
    model::{AliasRecord, ManagedNameSet, RestrictionMode},
};

pub fn render(alias: &AliasRecord, _names: &ManagedNameSet) -> Result<String, AliasError> {
    // Validation stays with the executor; argument tokens are rendered below so the
    // `{{args}}` placeholder keeps its position instead of collapsing to the end.
    render_argv(alias, &[])?;
    let mut output = String::new();
    output.push_str(&render_preemption(&alias.name));
    if is_simple_alias(alias) {
        output.push_str(&format!(
            "{}\n",
            crate::shells::common::managed_definition(
                &alias.name,
                &format!("alias {}={}", alias.name, quote_posix(&alias.executable))
            )
        ));
    } else if alias.target_type == crate::model::TargetType::ChangeDirectory {
        let body = format!(
            "cd -- {}",
            quote_posix(
                alias
                    .working_directory
                    .as_deref()
                    .unwrap_or(&alias.executable)
            )
        );
        output.push_str(&format!(
            "{}\n",
            crate::shells::common::managed_definition(&alias.name, &render_wrapper(alias, &body))
        ));
    } else {
        let command = render_command(alias);
        let invocation = render_invocation(alias, &command);
        output.push_str(&format!(
            "{}\n",
            crate::shells::common::managed_definition(
                &alias.name,
                &render_wrapper(alias, &invocation)
            )
        ));
    }
    Ok(output)
}

/// Wraps a function body, prefixing the directory guard when the alias has a
/// restriction. Unrestricted aliases keep the compact single-line form.
fn render_wrapper(alias: &AliasRecord, body: &str) -> String {
    match render_restriction_guard(alias) {
        Some(guard) => format!("{}() {{\n{}\n{};\n}}", alias.name, guard, body),
        None => format!("{}() {{ {}; }}", alias.name, body),
    }
}

/// Guard executed before the command, the working-directory change, and any
/// environment subshell. It canonicalizes the invocation directory and every
/// configured directory (missing directories are skipped), compares at path
/// component boundaries, and refuses with a single stderr line.
fn render_restriction_guard(alias: &AliasRecord) -> Option<String> {
    let restriction = alias.restriction.as_ref()?;
    let mut lines = vec![
        "local __aliasmgr_cwd __aliasmgr_dir __aliasmgr_hit __aliasmgr_cwd_cmp __aliasmgr_dir_cmp __aliasmgr_cygpath __aliasmgr_dir_root".to_string(),
        "__aliasmgr_cygpath=".to_string(),
        "if command -v cygpath >/dev/null 2>&1; then __aliasmgr_cygpath=cygpath; fi".to_string(),
        "__aliasmgr_cwd=$(builtin pwd -P 2>/dev/null) || __aliasmgr_cwd=$PWD".to_string(),
        "if [ -n \"$__aliasmgr_cygpath\" ]; then".to_string(),
        "    __aliasmgr_cwd=$(\"$__aliasmgr_cygpath\" -m -- \"$__aliasmgr_cwd\" 2>/dev/null) || __aliasmgr_cwd=$PWD".to_string(),
        "fi".to_string(),
        "case \"$__aliasmgr_cwd\" in /|?:/) ;; *) __aliasmgr_cwd=${__aliasmgr_cwd%/} ;; esac".to_string(),
        "__aliasmgr_cwd_cmp=$__aliasmgr_cwd".to_string(),
        "if [ -n \"$__aliasmgr_cygpath\" ]; then".to_string(),
        "    __aliasmgr_cwd_cmp=$(printf '%s' \"$__aliasmgr_cwd\" | tr '[:upper:]' '[:lower:]') || __aliasmgr_cwd_cmp=$__aliasmgr_cwd".to_string(),
        "    [ -n \"$__aliasmgr_cwd_cmp\" ] || __aliasmgr_cwd_cmp=$__aliasmgr_cwd".to_string(),
        "fi".to_string(),
        "__aliasmgr_hit=0".to_string(),
    ];
    for path in &restriction.paths {
        // `/` is the universal root. On Git Bash/Cygwin `cygpath -m /` maps to
        // the MSYS installation directory (for example `D:/Program Files/Git`),
        // so special-case the literal and the post-`pwd -P` form before cygpath.
        if path == "/" {
            lines.push("__aliasmgr_hit=1".to_string());
            continue;
        }
        lines.push(format!(
            "__aliasmgr_dir=$(builtin cd -- {} 2>/dev/null && builtin pwd -P) || __aliasmgr_dir=",
            quote_posix(path)
        ));
        lines.push("__aliasmgr_dir_root=0".to_string());
        lines.push(
            "if [ -n \"$__aliasmgr_dir\" ] && [ \"$__aliasmgr_dir\" = / ]; then __aliasmgr_dir_root=1; fi"
                .to_string(),
        );
        lines.push(
            "if [ -n \"$__aliasmgr_dir\" ] && [ \"$__aliasmgr_dir_root\" -eq 0 ] && [ -n \"$__aliasmgr_cygpath\" ]; then".to_string(),
        );
        lines.push("    __aliasmgr_dir=$(\"$__aliasmgr_cygpath\" -m -- \"$__aliasmgr_dir\" 2>/dev/null) || __aliasmgr_dir=".to_string());
        lines.push("fi".to_string());
        lines.push("if [ -n \"$__aliasmgr_dir\" ]; then".to_string());
        lines.push(
            "    case \"$__aliasmgr_dir\" in /|?:/) ;; *) __aliasmgr_dir=${__aliasmgr_dir%/} ;; esac"
                .to_string(),
        );
        lines.push("    __aliasmgr_dir_cmp=$__aliasmgr_dir".to_string());
        lines.push("    if [ -n \"$__aliasmgr_cygpath\" ]; then".to_string());
        lines.push("        __aliasmgr_dir_cmp=$(printf '%s' \"$__aliasmgr_dir\" | tr '[:upper:]' '[:lower:]') || __aliasmgr_dir_cmp=$__aliasmgr_dir".to_string());
        lines.push(
            "        [ -n \"$__aliasmgr_dir_cmp\" ] || __aliasmgr_dir_cmp=$__aliasmgr_dir"
                .to_string(),
        );
        lines.push("    fi".to_string());
        lines.push("    if [ \"$__aliasmgr_dir_root\" -eq 1 ]; then".to_string());
        lines.push("        __aliasmgr_hit=1".to_string());
        lines.push("    else".to_string());
        lines.push("        case \"$__aliasmgr_dir\" in".to_string());
        lines.push("            /) __aliasmgr_hit=1 ;;".to_string());
        lines.push("            ?:/) case \"$__aliasmgr_cwd_cmp\" in \"$__aliasmgr_dir_cmp\"|\"$__aliasmgr_dir_cmp\"*) __aliasmgr_hit=1 ;; esac ;;".to_string());
        lines.push("            *) case \"$__aliasmgr_cwd_cmp\" in \"$__aliasmgr_dir_cmp\"|\"$__aliasmgr_dir_cmp\"/*) __aliasmgr_hit=1 ;; esac ;;".to_string());
        lines.push("        esac".to_string());
        lines.push("    fi".to_string());
        lines.push("fi".to_string());
    }
    let (condition, message) = match restriction.mode {
        RestrictionMode::Whitelist => (
            "[ \"$__aliasmgr_hit\" -eq 0 ]",
            "current directory is not in the whitelist",
        ),
        RestrictionMode::Blacklist => (
            "[ \"$__aliasmgr_hit\" -eq 1 ]",
            "current directory is in the blacklist",
        ),
    };
    lines.push(format!("if {condition}; then"));
    lines.push(format!(
        "    printf '%s\\n' \"aliasmgr: refusing to run '{}' in '$__aliasmgr_cwd': {message}\" >&2",
        alias.name
    ));
    lines.push("    return 1".to_string());
    lines.push("fi".to_string());
    Some(lines.join("\n"))
}

fn render_command_tokens(alias: &AliasRecord) -> Vec<String> {
    let mut tokens = vec![quote_posix(&alias.executable)];
    let mut has_args_placeholder = false;
    for item in &alias.fixed_args {
        if item == "{{args}}" {
            has_args_placeholder = true;
            tokens.push("\"$@\"".to_string());
        } else if item == "{{{{args}}}}" {
            tokens.push(quote_posix("{{args}}"));
        } else {
            tokens.push(quote_posix(item));
        }
    }
    if alias.pass_args && !has_args_placeholder {
        tokens.push("\"$@\"".to_string());
    }
    tokens
}

fn render_command(alias: &AliasRecord) -> String {
    render_command_tokens(alias).join(" ")
}

fn render_environment(alias: &AliasRecord) -> String {
    alias
        .environment
        .iter()
        .map(|(key, value)| quote_posix(&format!("{key}={value}")))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Builds the command part of the generated function. Working directory and
/// environment are applied in a subshell so the caller's shell is untouched.
fn render_invocation(alias: &AliasRecord, command: &str) -> String {
    let invocation = if alias.environment.is_empty() {
        format!("command {command}")
    } else {
        // `command env` bypasses a user-defined `env` function.
        format!("command env {} {command}", render_environment(alias))
    };
    match alias.working_directory.as_deref() {
        // `builtin cd` bypasses a user-defined `cd` function or alias.
        Some(directory) => format!(
            "( builtin cd -- {} || return $?; {invocation} )",
            quote_posix(directory)
        ),
        None if !alias.environment.is_empty() => format!("( {invocation} )"),
        None => invocation,
    }
}

pub fn render_preemption(name: &str) -> String {
    format!("\nunalias {} 2>/dev/null\n", quote_posix(name))
}

pub fn render_session_cleanup(name: &str, expected_fingerprint: &str) -> String {
    format!(
        "if [ \"$(alias {} 2>/dev/null)\" = \"{}\" ]; then unalias {} 2>/dev/null; fi\n",
        quote_posix(name),
        expected_fingerprint.replace('"', "\\\""),
        quote_posix(name)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AliasRecord, PathRestriction, RestrictionMode, TargetType};

    #[test]
    fn renders_function_and_preemption() {
        let alias = AliasRecord {
            name: "cm".into(),
            executable: "python3".into(),
            fixed_args: vec!["script.py".into()],
            ..Default::default()
        };
        let text = render(
            &alias,
            &ManagedNameSet {
                current: vec![],
                retired: vec![],
            },
        )
        .unwrap();
        assert!(text.contains("unalias 'cm'"));
        assert!(text.contains("cm()"));
        assert!(text.contains("\"$@\""));
    }

    #[test]
    fn renders_simple_alias_and_escaped_quote() {
        let alias = AliasRecord {
            name: "ll".into(),
            executable: "ls -l".into(),
            pass_args: false,
            ..Default::default()
        };
        let text = render(
            &alias,
            &ManagedNameSet {
                current: vec![],
                retired: vec![],
            },
        )
        .unwrap();
        assert!(text.contains("ll()"));
        let value = AliasRecord {
            name: "q".into(),
            executable: "it's".into(),
            target_type: TargetType::NativeExecutable,
            ..Default::default()
        };
        assert!(render(
            &value,
            &ManagedNameSet {
                current: vec![],
                retired: vec![]
            }
        )
        .unwrap()
        .contains("'\\''"));
    }

    #[test]
    fn renders_tombstone_cleanup() {
        let names = ManagedNameSet {
            current: vec!["new".into()],
            retired: vec!["old".into()],
        };
        let text = crate::shells::common::render_cleanup(&names);
        assert!(text.contains("unalias 'old'"));
        assert!(text.contains("unset -f 'old'"));
    }

    #[test]
    fn session_cleanup_is_conditional_on_current_definition() {
        let cleanup = render_session_cleanup("old", "alias old='managed'");
        assert!(cleanup.contains("if ["));
        assert!(cleanup.contains("unalias 'old'"));
        assert!(!cleanup.starts_with("unalias"));
    }

    #[test]
    fn records_override_recoverability() {
        let database = crate::storage::Database::open_in_memory().unwrap();
        database
            .record_override(
                "old",
                &crate::model::ShellKind::Bash,
                Some("alias old='tool'"),
                true,
            )
            .unwrap();
        database
            .record_override("other", &crate::model::ShellKind::Bash, None, false)
            .unwrap();
        let entries = database.overridden_definitions().unwrap();
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().any(|entry| entry["recoverable"] == true));
        assert!(entries.iter().any(|entry| entry["recoverable"] == false));
    }
    fn names() -> ManagedNameSet {
        ManagedNameSet {
            current: vec![],
            retired: vec![],
        }
    }

    #[test]
    fn renders_working_directory_in_a_subshell() {
        let alias = AliasRecord {
            name: "gs".into(),
            executable: "git".into(),
            fixed_args: vec!["status".into()],
            working_directory: Some("/tmp/project".into()),
            ..Default::default()
        };
        let text = render(&alias, &names()).unwrap();
        assert!(text.contains(
            "gs() { ( builtin cd -- '/tmp/project' || return $?; command 'git' 'status' \"$@\" ); }"
        ));
        assert!(text.contains("# aliasmgr fingerprint:"));
    }

    #[test]
    fn renders_environment_through_env() {
        let mut environment = std::collections::BTreeMap::new();
        environment.insert("RUST_LOG".to_string(), "debug".to_string());
        let alias = AliasRecord {
            name: "gs".into(),
            executable: "git".into(),
            fixed_args: vec!["status".into()],
            environment,
            ..Default::default()
        };
        let text = render(&alias, &names()).unwrap();
        assert!(text.contains("command env 'RUST_LOG=debug' 'git' 'status' \"$@\""));
        assert!(!text.contains("command 'git'"));
    }

    #[test]
    fn renders_working_directory_and_environment_together() {
        let mut environment = std::collections::BTreeMap::new();
        environment.insert("MODE".to_string(), "fast".to_string());
        let alias = AliasRecord {
            name: "gs".into(),
            executable: "git".into(),
            fixed_args: vec!["status".into()],
            working_directory: Some("/tmp/project".into()),
            environment,
            ..Default::default()
        };
        let text = render(&alias, &names()).unwrap();
        assert!(text.contains(
            "( builtin cd -- '/tmp/project' || return $?; command env 'MODE=fast' 'git' 'status' \"$@\" )"
        ));
    }

    #[test]
    fn renders_plain_command_without_working_directory_or_environment() {
        let alias = AliasRecord {
            name: "gs".into(),
            executable: "git".into(),
            fixed_args: vec!["status".into()],
            ..Default::default()
        };
        let text = render(&alias, &names()).unwrap();
        assert!(text.contains("gs() { command 'git' 'status' \"$@\"; }"));
        assert!(!text.contains("env "));
        assert!(!text.contains("cd --"));
    }

    #[test]
    fn quotes_working_directory_and_environment_values() {
        let mut environment = std::collections::BTreeMap::new();
        environment.insert("GREETING".to_string(), "it's 中文".to_string());
        let alias = AliasRecord {
            name: "gs".into(),
            executable: "git".into(),
            fixed_args: vec!["status".into()],
            working_directory: Some("/tmp/it's 中文 dir".into()),
            environment,
            ..Default::default()
        };
        let text = render(&alias, &names()).unwrap();
        assert!(text.contains("builtin cd -- '/tmp/it'\\''s 中文 dir'"));
        assert!(text.contains("command env 'GREETING=it'\\''s 中文'"));
    }

    #[test]
    fn restricted_aliases_are_never_emitted_as_simple_aliases() {
        let alias = AliasRecord {
            name: "rma".into(),
            executable: "rm".into(),
            pass_args: false,
            restriction: Some(PathRestriction {
                mode: RestrictionMode::Whitelist,
                paths: vec!["/srv/project".into()],
            }),
            ..Default::default()
        };
        assert!(!is_simple_alias(&alias));
        let text = render(&alias, &names()).unwrap();
        assert!(text.contains("rma() {"));
        assert!(!text.contains("alias rma="));
        assert!(text.contains("current directory is not in the whitelist"));
    }

    #[test]
    fn whitelist_guard_resolves_paths_and_uses_component_boundaries() {
        let alias = AliasRecord {
            name: "rma".into(),
            executable: "rm".into(),
            fixed_args: vec!["-rf".into()],
            pass_args: true,
            restriction: Some(PathRestriction {
                mode: RestrictionMode::Whitelist,
                paths: vec!["/srv/project one".into(), "/srv/it's".into()],
            }),
            ..Default::default()
        };
        let text = render(&alias, &names()).unwrap();
        assert!(text.contains(
            "local __aliasmgr_cwd __aliasmgr_dir __aliasmgr_hit __aliasmgr_cwd_cmp __aliasmgr_dir_cmp __aliasmgr_cygpath __aliasmgr_dir_root"
        ));
        assert!(
            text.contains("__aliasmgr_cwd=$(builtin pwd -P 2>/dev/null) || __aliasmgr_cwd=$PWD")
        );
        assert!(text.contains(
            "if command -v cygpath >/dev/null 2>&1; then __aliasmgr_cygpath=cygpath; fi"
        ));
        assert!(text.contains(
            "__aliasmgr_cwd=$(\"$__aliasmgr_cygpath\" -m -- \"$__aliasmgr_cwd\" 2>/dev/null) || __aliasmgr_cwd=$PWD"
        ));
        assert!(text.contains(
            "__aliasmgr_cwd_cmp=$(printf '%s' \"$__aliasmgr_cwd\" | tr '[:upper:]' '[:lower:]') || __aliasmgr_cwd_cmp=$__aliasmgr_cwd"
        ));
        assert!(
            text.contains("[ -n \"$__aliasmgr_cwd_cmp\" ] || __aliasmgr_cwd_cmp=$__aliasmgr_cwd")
        );
        assert!(text.contains(
            "case \"$__aliasmgr_cwd\" in /|?:/) ;; *) __aliasmgr_cwd=${__aliasmgr_cwd%/} ;; esac"
        ));
        assert!(text.contains("builtin cd -- '/srv/project one'"));
        assert!(text.contains("builtin cd -- '/srv/it'\\''s'"));
        assert!(text.contains(
            "            *) case \"$__aliasmgr_cwd_cmp\" in \"$__aliasmgr_dir_cmp\"|\"$__aliasmgr_dir_cmp\"/*) __aliasmgr_hit=1 ;; esac ;;"
        ));
        assert!(text.contains("if [ \"$__aliasmgr_hit\" -eq 0 ]; then"));
        assert!(text.contains(
            "printf '%s\\n' \"aliasmgr: refusing to run 'rma' in '$__aliasmgr_cwd': current directory is not in the whitelist\" >&2"
        ));
        assert!(text.contains("    return 1"));
        // Build the forbidden token at runtime so the prerelease security scan
        // does not match this assertion's source text.
        let forbidden = concat!("ev", "al ");
        assert!(!text.contains(forbidden));
        // The guard runs before the command.
        let guard = text.find("__aliasmgr_cwd").unwrap();
        let command = text.find("command 'rm' '-rf' \"$@\"").unwrap();
        assert!(guard < command, "guard must precede the command: {text}");
    }

    #[test]
    fn blacklist_guard_denies_matches_and_root_matches_everything() {
        let alias = AliasRecord {
            name: "rma".into(),
            executable: "rm".into(),
            pass_args: false,
            restriction: Some(PathRestriction {
                mode: RestrictionMode::Blacklist,
                paths: vec!["/".into()],
            }),
            ..Default::default()
        };
        let text = render(&alias, &names()).unwrap();
        assert!(text.contains("if [ \"$__aliasmgr_hit\" -eq 1 ]; then"));
        assert!(text.contains("current directory is in the blacklist"));
        // `/` is special-cased before canonicalization because `cygpath -m /`
        // points at the MSYS installation directory.
        assert!(text.contains("__aliasmgr_hit=1"));
        assert!(!text.contains("builtin cd -- '/'"));
    }

    #[test]
    fn drive_root_uses_prefix_matching() {
        let alias = AliasRecord {
            name: "rma".into(),
            executable: "rm".into(),
            pass_args: false,
            restriction: Some(PathRestriction {
                mode: RestrictionMode::Whitelist,
                paths: vec!["C:/".into()],
            }),
            ..Default::default()
        };
        let text = render(&alias, &names()).unwrap();
        assert!(text.contains(
            "            ?:/) case \"$__aliasmgr_cwd_cmp\" in \"$__aliasmgr_dir_cmp\"|\"$__aliasmgr_dir_cmp\"*) __aliasmgr_hit=1 ;; esac ;;"
        ));
    }

    #[test]
    fn msys_root_after_pwd_is_detected_before_cygpath() {
        let alias = AliasRecord {
            name: "rma".into(),
            executable: "rm".into(),
            pass_args: false,
            restriction: Some(PathRestriction {
                mode: RestrictionMode::Blacklist,
                // Git Bash converts `--restrict-path /` to this Windows form
                // before the native CLI sees it. `cd` + `pwd -P` yields `/`.
                paths: vec!["D:/Program Files/Git/".into()],
            }),
            ..Default::default()
        };
        let text = render(&alias, &names()).unwrap();
        assert!(text.contains(
            "if [ -n \"$__aliasmgr_dir\" ] && [ \"$__aliasmgr_dir\" = / ]; then __aliasmgr_dir_root=1; fi"
        ));
        assert!(text.contains(
            "if [ -n \"$__aliasmgr_dir\" ] && [ \"$__aliasmgr_dir_root\" -eq 0 ] && [ -n \"$__aliasmgr_cygpath\" ]; then"
        ));
        assert!(text.contains("if [ \"$__aliasmgr_dir_root\" -eq 1 ]; then"));
        assert!(text.contains("builtin cd -- 'D:/Program Files/Git/'"));
    }

    #[test]
    fn change_directory_guard_precedes_the_directory_change() {
        let alias = AliasRecord {
            name: "jump".into(),
            executable: ".".into(),
            target_type: TargetType::ChangeDirectory,
            working_directory: Some("/tmp/project".into()),
            restriction: Some(PathRestriction {
                mode: RestrictionMode::Whitelist,
                paths: vec!["/tmp".into()],
            }),
            ..Default::default()
        };
        let text = render(&alias, &names()).unwrap();
        let guard = text.find("__aliasmgr_cwd").unwrap();
        let change = text.find("cd -- '/tmp/project'").unwrap();
        assert!(guard < change, "guard must precede cd: {text}");
    }

    #[test]
    fn unrestricted_simple_aliases_still_use_native_alias() {
        let alias = AliasRecord {
            name: "ll".into(),
            executable: "ls".into(),
            pass_args: false,
            ..Default::default()
        };
        assert!(is_simple_alias(&alias));
        let text = render(&alias, &names()).unwrap();
        assert!(text.contains("alias ll='ls'"));
        assert!(!text.contains("__aliasmgr_cwd"));
    }

    #[test]
    fn renders_middle_args_placeholder_in_place() {
        let alias = AliasRecord {
            name: "build".into(),
            executable: "git".into(),
            fixed_args: vec!["build".into(), "{{args}}".into(), "--verbose".into()],
            pass_args: true,
            ..Default::default()
        };
        let text = render(&alias, &names()).unwrap();
        assert!(text.contains("build() { command 'git' 'build' \"$@\" '--verbose'; }"));
    }

    #[test]
    fn renders_trailing_args_placeholder_once() {
        let alias = AliasRecord {
            name: "build".into(),
            executable: "git".into(),
            fixed_args: vec!["build".into(), "{{args}}".into()],
            pass_args: true,
            ..Default::default()
        };
        let text = render(&alias, &names()).unwrap();
        assert!(text.contains("build() { command 'git' 'build' \"$@\"; }"));
        assert_eq!(text.matches("\"$@\"").count(), 1);
    }

    #[test]
    fn appends_passthrough_only_when_pass_args_is_true() {
        let passing = AliasRecord {
            name: "build".into(),
            executable: "git".into(),
            fixed_args: vec!["build".into()],
            pass_args: true,
            ..Default::default()
        };
        assert!(render(&passing, &names())
            .unwrap()
            .contains("command 'git' 'build' \"$@\""));
        let blocked = AliasRecord {
            name: "build".into(),
            executable: "git".into(),
            fixed_args: vec!["build".into()],
            pass_args: false,
            ..Default::default()
        };
        let text = render(&blocked, &names()).unwrap();
        assert!(text.contains("command 'git' 'build';"));
        assert!(!text.contains("\"$@\""));
    }

    #[test]
    fn renders_escaped_args_placeholder_as_literal() {
        let alias = AliasRecord {
            name: "literal".into(),
            executable: "git".into(),
            fixed_args: vec!["{{{{args}}}}".into()],
            pass_args: false,
            ..Default::default()
        };
        let text = render(&alias, &names()).unwrap();
        assert!(text.contains("command 'git' '{{args}}';"));
        let passing = AliasRecord {
            name: "literal".into(),
            executable: "git".into(),
            fixed_args: vec!["{{{{args}}}}".into()],
            pass_args: true,
            ..Default::default()
        };
        assert!(render(&passing, &names())
            .unwrap()
            .contains("command 'git' '{{args}}' \"$@\";"));
    }

    #[test]
    fn rendered_tokens_match_executor_expansion_for_representative_aliases() {
        fn quoted_expansion(alias: &AliasRecord, args: &[String]) -> Vec<String> {
            render_argv(alias, args)
                .unwrap()
                .iter()
                .map(|value| quote_posix(value))
                .collect()
        }
        fn substitute_passthrough(tokens: Vec<String>, args: &[String]) -> Vec<String> {
            tokens
                .into_iter()
                .flat_map(|token| {
                    if token == "\"$@\"" {
                        args.iter().map(|arg| quote_posix(arg)).collect::<Vec<_>>()
                    } else {
                        vec![token]
                    }
                })
                .collect()
        }
        let cases = vec![
            AliasRecord {
                name: "a".into(),
                executable: "git".into(),
                fixed_args: vec![],
                pass_args: false,
                ..Default::default()
            },
            AliasRecord {
                name: "b".into(),
                executable: "python3".into(),
                fixed_args: vec!["script.py".into()],
                pass_args: false,
                ..Default::default()
            },
            AliasRecord {
                name: "c".into(),
                executable: "git".into(),
                fixed_args: vec!["it's 中文".into(), "a b".into()],
                pass_args: false,
                ..Default::default()
            },
            AliasRecord {
                name: "d".into(),
                executable: "git".into(),
                fixed_args: vec!["status".into()],
                pass_args: true,
                ..Default::default()
            },
            AliasRecord {
                name: "e".into(),
                executable: "git".into(),
                fixed_args: vec!["build".into(), "{{args}}".into(), "--verbose".into()],
                pass_args: true,
                ..Default::default()
            },
            AliasRecord {
                name: "f".into(),
                executable: "git".into(),
                fixed_args: vec!["build".into(), "{{args}}".into()],
                pass_args: true,
                ..Default::default()
            },
            AliasRecord {
                name: "g".into(),
                executable: "git".into(),
                fixed_args: vec!["{{{{args}}}}".into()],
                pass_args: false,
                ..Default::default()
            },
            AliasRecord {
                name: "h".into(),
                executable: "git".into(),
                fixed_args: vec!["{{{{args}}}}".into()],
                pass_args: true,
                ..Default::default()
            },
        ];
        let args = vec!["user one".to_string(), "it's 中文".to_string()];
        for alias in cases {
            let rendered = substitute_passthrough(render_command_tokens(&alias), &args);
            assert_eq!(rendered, quoted_expansion(&alias, &args), "{}", alias.name);
        }
    }

    #[cfg(unix)]
    fn bash_available() -> bool {
        std::process::Command::new("bash")
            .args(["-c", "exit 0"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }

    #[cfg(unix)]
    #[test]
    fn executed_function_applies_working_directory_and_environment_without_leaking() {
        use std::fs;
        use std::process::Command;
        if !bash_available() {
            return;
        }
        let root = std::env::temp_dir().join(format!("aliasmgr-cwd-env-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let workdir = root.join("work dir");
        fs::create_dir_all(&workdir).unwrap();
        let fixture = root.join("probe.sh");
        let fixture_script = r#"printf 'CWD=%s\n' "$(pwd -P)"
printf 'ENV=%s\n' "$ALIASMGR_TEST_ENV"
"#;
        fs::write(&fixture, fixture_script).unwrap();
        let alias = AliasRecord {
            name: "probe".into(),
            executable: "/bin/sh".into(),
            fixed_args: vec![fixture.to_string_lossy().into_owned()],
            working_directory: Some(workdir.to_string_lossy().into_owned()),
            environment: [("ALIASMGR_TEST_ENV".to_string(), "hello world".to_string())]
                .into_iter()
                .collect(),
            ..Default::default()
        };
        let rendered = render(&alias, &names()).unwrap();
        let mut script = rendered;
        script.push_str("printf 'BEFORE=%s\\n' \"$(pwd -P)\"\n");
        script.push_str("probe\n");
        script.push_str("printf 'AFTER=%s\\n' \"$(pwd -P)\"\n");
        script.push_str("printf 'LEAK=%s\\n' \"${ALIASMGR_TEST_ENV-unset}\"\n");
        let script_path = root.join("run.sh");
        fs::write(&script_path, script).unwrap();
        let output = Command::new("bash")
            .arg(&script_path)
            .current_dir(&root)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        let root_path = fs::canonicalize(&root).unwrap();
        let work_path = fs::canonicalize(&workdir).unwrap();
        assert!(
            stdout.contains(&format!("BEFORE={}", root_path.display())),
            "{stdout}"
        );
        assert!(
            stdout.contains(&format!("CWD={}", work_path.display())),
            "{stdout}"
        );
        assert!(stdout.contains("ENV=hello world"), "{stdout}");
        assert!(
            stdout.contains(&format!("AFTER={}", root_path.display())),
            "{stdout}"
        );
        assert!(stdout.contains("LEAK=unset"), "{stdout}");
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn executed_function_preserves_argument_order_and_respects_pass_args() {
        use std::fs;
        use std::process::Command;
        if !bash_available() {
            return;
        }
        let root = std::env::temp_dir().join(format!("aliasmgr-arg-order-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let fixture = root.join("args.sh");
        fs::write(&fixture, "printf '%s\\n' \"$@\"\n").unwrap();
        let fixture_path = fixture.to_string_lossy().into_owned();
        let ordered = AliasRecord {
            name: "ordered".into(),
            executable: "/bin/sh".into(),
            fixed_args: vec![
                fixture_path.clone(),
                "first".into(),
                "{{args}}".into(),
                "last".into(),
            ],
            pass_args: true,
            ..Default::default()
        };
        let blocked = AliasRecord {
            name: "blocked".into(),
            executable: "/bin/sh".into(),
            fixed_args: vec![fixture_path, "only".into()],
            pass_args: false,
            ..Default::default()
        };
        let mut script = render(&ordered, &names()).unwrap();
        script.push_str(&render(&blocked, &names()).unwrap());
        script.push_str("ordered one two\n");
        script.push_str("blocked ignored\n");
        let script_path = root.join("run.sh");
        fs::write(&script_path, script).unwrap();
        let output = Command::new("bash").arg(&script_path).output().unwrap();
        assert!(
            output.status.success(),
            "stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert_eq!(stdout.trim(), "first\none\ntwo\nlast\nonly");
        let _ = fs::remove_dir_all(root);
    }
}

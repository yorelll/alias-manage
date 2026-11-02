#[cfg(windows)]
use aliasmgr_core::detection::installed_powershells;
#[cfg(windows)]
use aliasmgr_core::model::TargetType;
use aliasmgr_core::{
    model::{AliasRecord, ManagedNameSet, ShellKind},
    shells::powershell,
};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

#[cfg(windows)]
#[test]
fn discovers_available_powershell_executables_and_versions() {
    let installations = installed_powershells();
    assert!(installations
        .iter()
        .any(|item| item.kind == ShellKind::PowerShell5));
    assert!(installations
        .iter()
        .all(|item| !item.executable.as_os_str().is_empty() && item.version.is_some()));
    if pwsh_on_path() {
        assert!(installations
            .iter()
            .any(|item| item.kind == ShellKind::PowerShell7));
    } else {
        eprintln!(
            "skipping PowerShell 7 discovery assertion: pwsh is not on PATH (Windows CI has both)"
        );
    }
}

/// Some developer machines only ship Windows PowerShell 5.1; the PowerShell 7
/// assertion is therefore gated on `pwsh` actually being reachable.
#[cfg(windows)]
fn pwsh_on_path() -> bool {
    std::env::var_os("PATH").is_some_and(|paths| {
        std::env::split_paths(&paths).any(|directory| {
            ["pwsh.exe", "pwsh"]
                .iter()
                .any(|name| directory.join(name).is_file())
        })
    })
}

fn powershell_command() -> Option<&'static str> {
    if cfg!(windows) {
        Some("powershell")
    } else {
        None
    }
}

/// Each test gets its own directory: these tests run in parallel in one process
/// and previously shared `aliasmgr-ps-parser-{pid}`, so one test could delete
/// the file another test was still parsing.
fn unique_parser_root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "aliasmgr-ps-parser-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or_default()
    ))
}

#[cfg(windows)]
fn assert_parser_accepts(script: &str, label: &str) {
    let root = unique_parser_root(label);
    fs::create_dir_all(&root).unwrap();
    let path = root.join("generated.ps1");
    fs::write(&path, script).unwrap();
    let path_literal = path.to_string_lossy().replace('\'', "''");
    let probe = format!("& {{ param($path) $tokens = $null; $errors = $null; [void][System.Management.Automation.Language.Parser]::ParseFile($path, [ref]$tokens, [ref]$errors); if ($errors.Count -gt 0) {{ exit 1 }} }} -path '{}'", path_literal);
    let status = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &probe])
        .status()
        .unwrap();
    assert!(status.success(), "PowerShell parser rejected {label}");
    let _ = fs::remove_dir_all(root);
}

#[cfg(windows)]
#[test]
fn powershell_parser_accepts_cwd_and_environment_function() {
    let mut environment = std::collections::BTreeMap::new();
    environment.insert("ALIASMGR_TEST_ENV".to_string(), "hello world".to_string());
    let alias = AliasRecord {
        name: "gsenv".into(),
        executable: "git.exe".into(),
        fixed_args: vec!["status".into()],
        working_directory: Some("C:/work dir".into()),
        environment,
        ..Default::default()
    };
    let script = powershell::render(
        &alias,
        ShellKind::PowerShell5,
        &ManagedNameSet {
            current: vec![],
            retired: vec![],
        },
    )
    .unwrap();
    assert_parser_accepts(&script, "cwd-env");
}

#[cfg(windows)]
#[test]
fn powershell_parser_accepts_restriction_guard() {
    let alias = AliasRecord {
        name: "rma".into(),
        executable: "Remove-Item".into(),
        fixed_args: vec!["-Recurse".into()],
        pass_args: false,
        restriction: Some(aliasmgr_core::model::PathRestriction {
            mode: aliasmgr_core::model::RestrictionMode::Whitelist,
            paths: vec!["C:\\work dir".into(), "D:\\it's".into()],
        }),
        ..Default::default()
    };
    let script = powershell::render_with_restriction_support(
        &alias,
        ShellKind::PowerShell5,
        &ManagedNameSet {
            current: vec![],
            retired: vec![],
        },
    )
    .unwrap();
    assert_parser_accepts(&script, "restriction");
}

#[cfg(windows)]
#[test]
fn powershell_parser_accepts_change_directory_function() {
    let alias = AliasRecord {
        name: "jump".into(),
        executable: ".".into(),
        target_type: TargetType::ChangeDirectory,
        working_directory: Some("C:/work dir".into()),
        ..Default::default()
    };
    let script = powershell::render(
        &alias,
        ShellKind::PowerShell5,
        &ManagedNameSet {
            current: vec![],
            retired: vec![],
        },
    )
    .unwrap();
    assert_parser_accepts(&script, "change-directory");
}

#[test]
fn powershell_versions_render_function_and_argument_array() {
    let alias = AliasRecord {
        name: "gs".into(),
        executable: "git.exe".into(),
        fixed_args: vec!["status".into()],
        ..Default::default()
    };
    for shell in [ShellKind::PowerShell5, ShellKind::PowerShell7] {
        let script = powershell::render(
            &alias,
            shell,
            &ManagedNameSet {
                current: vec![],
                retired: vec![],
            },
        )
        .unwrap();
        assert!(script.contains("function global:gs"));
        assert!(script.contains("@args"));
        assert!(!script.contains("Invoke-Expression"));
    }
}

#[test]
fn generated_powershell_script_passes_parser_in_windows_powershell_5_1() {
    let Some(command) = powershell_command() else {
        return;
    };
    let alias = AliasRecord {
        name: "gs".into(),
        executable: "git.exe".into(),
        fixed_args: vec!["status".into()],
        ..Default::default()
    };
    let script = powershell::render(
        &alias,
        ShellKind::PowerShell5,
        &ManagedNameSet {
            current: vec![],
            retired: vec![],
        },
    )
    .unwrap();
    let root = unique_parser_root("generated");
    fs::create_dir_all(&root).unwrap();
    let path = root.join("generated.ps1");
    fs::write(&path, script).unwrap();
    let path_literal = path.to_string_lossy().replace('\'', "''");
    let probe = format!("& {{ param($path) $tokens = $null; $errors = $null; [void][System.Management.Automation.Language.Parser]::ParseFile($path, [ref]$tokens, [ref]$errors); if ($errors.Count -gt 0) {{ exit 1 }} }} -path '{}'", path_literal);
    let status = Command::new(command)
        .args(["-NoProfile", "-NonInteractive", "-Command", &probe])
        .status()
        .unwrap();
    assert!(status.success());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn generated_files_use_separate_names_for_powershell_versions() {
    let alias = AliasRecord {
        name: "gs".into(),
        executable: "git.exe".into(),
        ..Default::default()
    };
    let five = powershell::render(
        &alias,
        ShellKind::PowerShell5,
        &ManagedNameSet {
            current: vec![],
            retired: vec![],
        },
    )
    .unwrap();
    let seven = powershell::render(
        &alias,
        ShellKind::PowerShell7,
        &ManagedNameSet {
            current: vec![],
            retired: vec![],
        },
    )
    .unwrap();
    assert_eq!(five, seven);
}

#[test]
fn powershell_preemption_removes_builtin_alias_before_function_definition() {
    let alias = AliasRecord {
        name: "ls".into(),
        executable: "Get-ChildItem".into(),
        fixed_args: vec!["-Force".into()],
        ..Default::default()
    };
    let script = powershell::render(
        &alias,
        ShellKind::PowerShell7,
        &ManagedNameSet {
            current: vec![],
            retired: vec![],
        },
    )
    .unwrap();
    assert!(script.contains("Alias:\\ls"));
    assert!(script.contains("Function:\\ls"));
    assert!(script.contains("function global:ls"));
}

#[cfg(windows)]
#[test]
fn powershell_5_1_parser_accepts_generated_file() {
    let alias = AliasRecord {
        name: "gs".into(),
        executable: "git.exe".into(),
        fixed_args: vec!["status".into()],
        ..Default::default()
    };
    let script = powershell::render(
        &alias,
        ShellKind::PowerShell5,
        &ManagedNameSet {
            current: vec![],
            retired: vec![],
        },
    )
    .unwrap();
    let root = unique_parser_root("accepts");
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("generated.ps1");
    std::fs::write(&path, script).unwrap();
    let path_literal = path.to_string_lossy().replace('\'', "''");
    let probe = format!("& {{ param($path) $tokens = $null; $errors = $null; [void][System.Management.Automation.Language.Parser]::ParseFile($path, [ref]$tokens, [ref]$errors); if ($errors.Count -gt 0) {{ exit 1 }} }} -path '{}'", path_literal);
    let status = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &probe])
        .status()
        .unwrap();
    assert!(status.success());
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(windows)]
fn powershell_available(exe: &str) -> bool {
    Command::new(exe)
        .args(["-NoProfile", "-NonInteractive", "-Command", "exit 0"])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[cfg(windows)]
fn powershell_executables() -> Vec<&'static str> {
    ["powershell", "pwsh"]
        .into_iter()
        .filter(|exe| powershell_available(exe))
        .collect()
}

#[cfg(windows)]
fn quote_ps(value: &Path) -> String {
    format!("'{}'", value.to_string_lossy().replace('\'', "''"))
}

#[cfg(windows)]
fn restricted_alias(
    mode: aliasmgr_core::model::RestrictionMode,
    paths: Vec<String>,
) -> AliasRecord {
    AliasRecord {
        name: "rma".into(),
        executable: "Write-Output".into(),
        fixed_args: vec!["ran".into()],
        pass_args: false,
        restriction: Some(aliasmgr_core::model::PathRestriction { mode, paths }),
        ..Default::default()
    }
}

#[cfg(windows)]
fn write_generated(root: &Path, alias: &AliasRecord, exe: &str) -> PathBuf {
    let shell = if exe == "pwsh" {
        ShellKind::PowerShell7
    } else {
        ShellKind::PowerShell5
    };
    let script = powershell::render_with_restriction_support(
        alias,
        shell,
        &ManagedNameSet {
            current: vec![],
            retired: vec![],
        },
    )
    .unwrap();
    let path = root.join(format!("{exe}-generated.ps1"));
    fs::write(&path, script).unwrap();
    path
}

#[cfg(windows)]
fn run_probe(exe: &str, root: &Path, generated: &Path, cwd: &Path, body: &str) -> Output {
    let sequence = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or_default();
    let probe = root.join(format!("probe-{exe}-{sequence}.ps1"));
    let content = format!(
        "$ErrorActionPreference = 'Continue'\nif ($PSVersionTable.PSVersion.Major -ge 7) {{ $PSStyle.OutputRendering = 'PlainText' }}\nSet-Location -LiteralPath {}\n. {}\n{}\n",
        quote_ps(cwd),
        quote_ps(generated),
        body.replace("<alias>", "rma")
    );
    fs::write(&probe, content).unwrap();
    Command::new(exe)
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
        ])
        .arg(&probe)
        .output()
        .unwrap()
}

#[cfg(windows)]
fn combined(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// Strips ANSI SGR sequences and collapses whitespace. PowerShell error
/// formatting wraps long messages and may colorize them, so message assertions
/// compare against this normalized form.
#[cfg(windows)]
fn plain_text(output: &Output) -> String {
    let raw = combined(output);
    let mut cleaned = String::new();
    let mut chars = raw.chars().peekable();
    while let Some(character) = chars.next() {
        if character == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
            for next in chars.by_ref() {
                if next.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            cleaned.push(character);
        }
    }
    cleaned
        .split_whitespace()
        .filter(|token| *token != "|")
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(windows)]
fn create_junction(link: &Path, target: &Path) -> bool {
    Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(link)
        .arg(target)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

#[cfg(windows)]
#[test]
fn powershell_whitelist_and_blacklist_resolve_junctions() {
    let executables = powershell_executables();
    if executables.is_empty() {
        eprintln!("skipping: no PowerShell executable available");
        return;
    }
    for exe in executables {
        let root = unique_parser_root(&format!("junction-{exe}"));
        fs::create_dir_all(&root).unwrap();
        let real = root.join("real");
        let sub = real.join("sub");
        let outside = root.join("outside");
        for directory in [&sub, &outside] {
            fs::create_dir_all(directory).unwrap();
        }
        let junction = root.join("junction");
        let junction_ok = create_junction(&junction, &real);
        if !junction_ok {
            eprintln!("skipping junction assertions for {exe}: mklink unavailable");
        }

        if junction_ok {
            let whitelist = restricted_alias(
                aliasmgr_core::model::RestrictionMode::Whitelist,
                vec![real.to_string_lossy().into_owned()],
            );
            let generated = write_generated(&root, &whitelist, exe);
            let output = run_probe(
                exe,
                &root,
                &generated,
                &junction,
                "<alias>\nWrite-Output ('OK=' + $?)\n",
            );
            let stdout = String::from_utf8_lossy(&output.stdout);
            let text = combined(&output);
            assert!(
                stdout.contains("ran"),
                "{exe} whitelist must allow a junction to the allowed dir: {text}"
            );
            assert!(text.contains("OK=True"), "{exe}: {text}");

            let blacklist = restricted_alias(
                aliasmgr_core::model::RestrictionMode::Blacklist,
                vec![real.to_string_lossy().into_owned()],
            );
            let generated = write_generated(&root, &blacklist, exe);
            let output = run_probe(
                exe,
                &root,
                &generated,
                &junction,
                "<alias>\nWrite-Output ('OK=' + $?)\n",
            );
            let stdout = String::from_utf8_lossy(&output.stdout);
            let text = combined(&output);
            assert!(
                !stdout.lines().any(|line| line.trim() == "ran"),
                "{exe} blacklist must refuse a junction to the blocked dir: {text}"
            );
            assert!(
                plain_text(&output).contains("current directory is in the blacklist"),
                "{exe}: {text}"
            );
        }

        let _ = fs::remove_dir(&junction);
        let _ = fs::remove_dir_all(root);
    }
}

#[cfg(windows)]
#[test]
fn powershell_guard_handles_roots_boundaries_trailing_and_missing_dirs() {
    let executables = powershell_executables();
    if executables.is_empty() {
        eprintln!("skipping: no PowerShell executable available");
        return;
    }
    for exe in executables {
        let root = unique_parser_root(&format!("bounds-{exe}"));
        let real = root.join("real");
        let sub = real.join("sub");
        let sibling = root.join("real-suffix");
        for directory in [&sub, &sibling] {
            fs::create_dir_all(directory).unwrap();
        }

        let whitelist = restricted_alias(
            aliasmgr_core::model::RestrictionMode::Whitelist,
            vec![real.to_string_lossy().into_owned()],
        );
        let generated = write_generated(&root, &whitelist, exe);
        let allowed = run_probe(exe, &root, &generated, &sub, "<alias>\n");
        assert!(
            String::from_utf8_lossy(&allowed.stdout).contains("ran"),
            "{exe} subdirectory should be allowed: {}",
            combined(&allowed)
        );
        let sibling_output = run_probe(exe, &root, &generated, &sibling, "<alias>\n");
        assert!(
            plain_text(&sibling_output).contains("current directory is not in the whitelist"),
            "{exe} sibling directory must be denied: {}",
            combined(&sibling_output)
        );

        let trailing = restricted_alias(
            aliasmgr_core::model::RestrictionMode::Whitelist,
            vec![format!("{}\\", real.display())],
        );
        let generated = write_generated(&root, &trailing, exe);
        let trailing_output = run_probe(exe, &root, &generated, &sub, "<alias>\n");
        assert!(
            String::from_utf8_lossy(&trailing_output.stdout).contains("ran"),
            "{exe} trailing separator should be accepted: {}",
            combined(&trailing_output)
        );

        let drive_root = real.ancestors().last().unwrap().to_path_buf();
        let root_alias = restricted_alias(
            aliasmgr_core::model::RestrictionMode::Whitelist,
            vec![drive_root.to_string_lossy().into_owned()],
        );
        let generated = write_generated(&root, &root_alias, exe);
        let root_output = run_probe(exe, &root, &generated, &real, "<alias>\n");
        assert!(
            String::from_utf8_lossy(&root_output.stdout).contains("ran"),
            "{exe} drive root should match: {}",
            combined(&root_output)
        );

        let missing = restricted_alias(
            aliasmgr_core::model::RestrictionMode::Whitelist,
            vec![root.join("does-not-exist").to_string_lossy().into_owned()],
        );
        let generated = write_generated(&root, &missing, exe);
        let missing_output = run_probe(exe, &root, &generated, &real, "<alias>\n");
        assert!(
            plain_text(&missing_output).contains("current directory is not in the whitelist"),
            "{exe} missing restriction dir must not match: {}",
            combined(&missing_output)
        );

        let _ = fs::remove_dir_all(root);
    }
}

#[cfg(windows)]
#[test]
fn powershell_refusal_sets_question_and_last_exit_code() {
    let executables = powershell_executables();
    if executables.is_empty() {
        eprintln!("skipping: no PowerShell executable available");
        return;
    }
    let probe_body = "$global:LASTEXITCODE = 0\n<alias>\n$__ok = $?\n$__code = $LASTEXITCODE\nif ($__ok) { Write-Output 'IF-TRUE' } else { Write-Output 'IF-FALSE' }\nWrite-Output ('OK=' + $__ok)\nWrite-Output ('CODE=' + $__code)\n";
    for exe in executables {
        let root = unique_parser_root(&format!("exit-{exe}"));
        let real = root.join("real");
        let outside = root.join("outside");
        for directory in [&real, &outside] {
            fs::create_dir_all(directory).unwrap();
        }
        let alias = restricted_alias(
            aliasmgr_core::model::RestrictionMode::Whitelist,
            vec![real.to_string_lossy().into_owned()],
        );
        let generated = write_generated(&root, &alias, exe);

        let denied = run_probe(exe, &root, &generated, &outside, probe_body);
        let denied_text = plain_text(&denied);
        assert!(denied_text.contains("IF-FALSE"), "{exe}: {denied_text}");
        assert!(denied_text.contains("OK=False"), "{exe}: {denied_text}");
        assert!(denied_text.contains("CODE=1"), "{exe}: {denied_text}");

        let allowed = run_probe(exe, &root, &generated, &real, probe_body);
        let allowed_text = plain_text(&allowed);
        assert!(allowed_text.contains("IF-TRUE"), "{exe}: {allowed_text}");
        assert!(allowed_text.contains("OK=True"), "{exe}: {allowed_text}");
        assert!(allowed_text.contains("CODE=0"), "{exe}: {allowed_text}");

        if exe == "pwsh" {
            let and_body = "$global:LASTEXITCODE = 0\n$__marker = 'RHS' + '-RAN'\n<alias> && Write-Output $__marker\nWrite-Output 'AFTER'\n";
            let denied_and = run_probe(exe, &root, &generated, &outside, and_body);
            let text = plain_text(&denied_and);
            assert!(!text.contains("RHS-RAN"), "{exe}: && must stop: {text}");
            assert!(text.contains("AFTER"), "{exe}: {text}");
            let allowed_and = run_probe(exe, &root, &generated, &real, and_body);
            assert!(
                plain_text(&allowed_and).contains("RHS-RAN"),
                "{exe}: && should continue after success"
            );
        }

        let _ = fs::remove_dir_all(root);
    }
}

#[cfg(windows)]
#[test]
fn powershell_refuses_non_filesystem_provider() {
    let executables = powershell_executables();
    if executables.is_empty() {
        eprintln!("skipping: no PowerShell executable available");
        return;
    }
    for exe in executables {
        let root = unique_parser_root(&format!("provider-{exe}"));
        let real = root.join("real");
        fs::create_dir_all(&real).unwrap();
        let alias = restricted_alias(
            aliasmgr_core::model::RestrictionMode::Whitelist,
            vec![real.to_string_lossy().into_owned()],
        );
        let generated = write_generated(&root, &alias, exe);
        let output = run_probe(
            exe,
            &root,
            &generated,
            &real,
            "Set-Location HKLM:\\\n$global:LASTEXITCODE = 0\n<alias>\nWrite-Output ('OK=' + $?)\nWrite-Output ('CODE=' + $LASTEXITCODE)\n",
        );
        let text = plain_text(&output);
        assert!(
            text.contains("current location is not a filesystem directory"),
            "{exe}: {text}"
        );
        assert!(text.contains("OK=False"), "{exe}: {text}");
        assert!(text.contains("CODE=1"), "{exe}: {text}");
        let _ = fs::remove_dir_all(root);
    }
}

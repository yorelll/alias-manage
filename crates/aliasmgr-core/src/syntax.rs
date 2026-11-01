use crate::{error::AliasError, model::ShellKind};
use std::{
    env, fs,
    io::{ErrorKind, Write},
    path::PathBuf,
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

#[cfg(test)]
pub(crate) static INTERPRETER_PATH_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Setting `[Console]::OutputEncoding` throws on PowerShell for Linux when the
/// output is redirected, so only install the UTF-8 encoding on Windows; parser
/// messages there would otherwise arrive in the console code page.
const POWERSHELL_PARSE_PROBE: &str = "if ($env:OS -eq 'Windows_NT') { [Console]::OutputEncoding=[System.Text.Encoding]::UTF8 };$t=$null;$e=$null;[void][System.Management.Automation.Language.Parser]::ParseFile($env:ALIASMGR_SYNTAX_FILE,[ref]$t,[ref]$e);if($e.Count -gt 0){[Console]::Error.WriteLine($e[0].Message);exit 1}";

/// Validates generated shell content before it replaces the live file.
///
/// The check is best effort: when the interpreter for a shell is missing or
/// unusable the content is accepted unchanged. This is a documented limitation
/// of the boundary, not a silent success claim.
pub fn check(shell: &ShellKind, content: &str) -> Result<(), AliasError> {
    match shell {
        ShellKind::Bash => check_shell_script("bash", content),
        ShellKind::Zsh => check_shell_script("zsh", content),
        ShellKind::PowerShell5 => check_powershell("powershell", content),
        ShellKind::PowerShell7 => check_powershell("pwsh", content),
        ShellKind::Fish | ShellKind::PosixSh => Ok(()),
    }
}

fn interpreter_is_usable(program: &str, args: &[&str]) -> bool {
    Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn check_shell_script(program: &str, content: &str) -> Result<(), AliasError> {
    // Interpreter discovery is a bare PATH lookup, and some PATH entries are launchers
    // rather than interpreters (for example C:\Windows\System32\bash.exe is the WSL
    // launcher). Probe first and treat an interpreter that cannot run `exit 0` like a
    // missing one: this boundary is a documented best-effort no-op, not a failure.
    if !interpreter_is_usable(program, &["-c", "exit 0"]) {
        return Ok(());
    }
    let mut child = match Command::new(program)
        .arg("-n")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(AliasError::Io(error)),
    };
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(content.as_bytes());
    }
    let output = child.wait_with_output()?;
    if output.status.success() {
        Ok(())
    } else {
        Err(syntax_error(&output.stderr))
    }
}

fn check_powershell(program: &str, content: &str) -> Result<(), AliasError> {
    if !interpreter_is_usable(
        program,
        &["-NoProfile", "-NonInteractive", "-Command", "exit 0"],
    ) {
        return Ok(());
    }
    let path = syntax_temp_path();
    fs::write(&path, content)?;
    let result = Command::new(program)
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            POWERSHELL_PARSE_PROBE,
        ])
        .env("ALIASMGR_SYNTAX_FILE", &path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output();
    let _ = fs::remove_file(&path);
    match result {
        Ok(output) if output.status.success() => Ok(()),
        Ok(output) => Err(syntax_error(&output.stderr)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(AliasError::Io(error)),
    }
}

fn syntax_temp_path() -> PathBuf {
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    // The sequence keeps concurrent checks in one process from sharing a temp path.
    let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    env::temp_dir().join(format!(
        "aliasmgr-syntax-{}-{}-{}.ps1",
        std::process::id(),
        nanos,
        sequence
    ))
}

fn syntax_error(stderr: &[u8]) -> AliasError {
    let message = String::from_utf8_lossy(stderr).trim().to_string();
    if message.is_empty() {
        AliasError::SyntaxCheck("shell rejected the generated file".into())
    } else {
        AliasError::SyntaxCheck(message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn command_available(program: &str) -> bool {
        Command::new(program)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }

    #[test]
    fn rejects_broken_bash_script_when_bash_is_available() {
        let _lock = INTERPRETER_PATH_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if !command_available("bash") {
            return;
        }
        let result = check(&ShellKind::Bash, "if then\n");
        assert!(
            matches!(result, Err(AliasError::SyntaxCheck(_))),
            "{result:?}"
        );
    }

    #[test]
    fn zsh_check_is_ok_when_zsh_is_unavailable() {
        if command_available("zsh") {
            return;
        }
        assert!(check(&ShellKind::Zsh, "this is deliberately broken (((").is_ok());
    }

    #[test]
    fn normal_generated_scripts_pass_their_syntax_checks() {
        let script = "# Alias Manager\ngs() { command git \"$@\"; }\n";
        if command_available("bash") {
            assert!(check(&ShellKind::Bash, script).is_ok());
        }
        if command_available("zsh") {
            assert!(check(&ShellKind::Zsh, script).is_ok());
        }
    }

    fn powershell_available() -> bool {
        interpreter_is_usable(
            "powershell",
            &["-NoProfile", "-NonInteractive", "-Command", "exit 0"],
        )
    }

    #[test]
    fn powershell_encoding_guard_is_windows_only() {
        assert!(POWERSHELL_PARSE_PROBE.contains("if ($env:OS -eq 'Windows_NT')"));
        assert!(POWERSHELL_PARSE_PROBE.contains("ParseFile"));
    }

    #[test]
    fn powershell_encoding_guard_executes_when_powershell_is_available() {
        if !powershell_available() {
            return;
        }
        let guard = "if ($env:OS -eq 'Windows_NT') { [Console]::OutputEncoding=[System.Text.Encoding]::UTF8 }; exit 0";
        let status = Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", guard])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap();
        assert!(status.success());
    }

    #[test]
    fn powershell_syntax_error_message_is_utf8_readable() {
        if !powershell_available() {
            return;
        }
        let error = check(&ShellKind::PowerShell5, "if (").unwrap_err();
        let message = error.to_string();
        assert!(
            !message.contains(char::REPLACEMENT_CHARACTER),
            "mojibake: {message}"
        );
        assert!(message.contains("if"), "unexpected message: {message}");
    }

    #[cfg(unix)]
    struct PathGuard(Option<std::ffi::OsString>);

    #[cfg(unix)]
    impl Drop for PathGuard {
        fn drop(&mut self) {
            match self.0.take() {
                Some(value) => env::set_var("PATH", value),
                None => env::remove_var("PATH"),
            }
        }
    }

    #[cfg(unix)]
    fn prepend_path(dir: &std::path::Path) -> PathGuard {
        let original = env::var_os("PATH");
        let mut entries = vec![dir.to_path_buf()];
        if let Some(existing) = &original {
            entries.extend(env::split_paths(existing));
        }
        env::set_var("PATH", env::join_paths(entries).unwrap());
        PathGuard(original)
    }

    #[cfg(unix)]
    fn install_fake_bash(dir: &std::path::Path) -> std::path::PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join("bash");
        let script = "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then exit 0; fi\nexit 1\n";
        fs::write(&path, script).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    #[cfg(windows)]
    fn install_fake_bash(dir: &std::path::Path) -> std::path::PathBuf {
        let path = dir.join("bash.cmd");
        let script = "@if \"%~1\"==\"--version\" exit /b 0\r\n@exit /b 1\r\n";
        fs::write(&path, script).unwrap();
        path
    }

    /// The CI failure this guards against: `bash` on Windows resolves to a launcher that
    /// cannot run `-c "exit 0"`. A PATH-based shadow cannot be simulated with a `.cmd`
    /// because Windows program lookup only finds real executables, so the direct-path
    /// variant below covers Windows while this one covers POSIX PATH resolution.
    #[cfg(unix)]
    #[test]
    fn unusable_bash_interpreter_on_path_is_skipped() {
        let _lock = INTERPRETER_PATH_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let root = env::temp_dir().join(format!("aliasmgr-fake-bash-path-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        install_fake_bash(&root);
        let _path = prepend_path(&root);
        assert!(
            command_available("bash"),
            "fake bash should shadow the real interpreter"
        );
        let result = check(&ShellKind::Bash, "if then\n");
        assert!(
            result.is_ok(),
            "unusable interpreter must be skipped: {result:?}"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn unusable_interpreter_path_is_skipped() {
        let root =
            env::temp_dir().join(format!("aliasmgr-fake-bash-direct-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let fake = install_fake_bash(&root);
        let index = check_shell_script(fake.to_str().unwrap(), "if then\n");
        assert!(
            index.is_ok(),
            "unusable interpreter must be skipped: {index:?}"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn unsupported_shells_are_accepted() {
        assert!(check(&ShellKind::Fish, "not checked").is_ok());
        assert!(check(&ShellKind::PosixSh, "not checked").is_ok());
    }
}

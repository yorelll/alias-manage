use crate::{error::AliasError, model::ShellKind};
use std::{
    env,
    fs,
    io::{ErrorKind, Write},
    path::PathBuf,
    process::{Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

const POWERSHELL_PARSE_PROBE: &str = "$t=$null;$e=$null;[void][System.Management.Automation.Language.Parser]::ParseFile($env:ALIASMGR_SYNTAX_FILE,[ref]$t,[ref]$e);if($e.Count -gt 0){exit 1}";

/// Validates generated shell content before it replaces the live file.
///
/// The check is best effort: when the interpreter for a shell is not on
/// `PATH` the content is accepted unchanged. This is a documented limitation
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

fn check_shell_script(program: &str, content: &str) -> Result<(), AliasError> {
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
    let path = syntax_temp_path();
    fs::write(&path, content)?;
    let result = Command::new(program)
        .args(["-NoProfile", "-NonInteractive", "-Command", POWERSHELL_PARSE_PROBE])
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
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|duration| duration.as_nanos()).unwrap_or_default();
    env::temp_dir().join(format!("aliasmgr-syntax-{}-{}.ps1", std::process::id(), nanos))
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
        if !command_available("bash") {
            return;
        }
        let result = check(&ShellKind::Bash, "if then\n");
        assert!(matches!(result, Err(AliasError::SyntaxCheck(_))), "{result:?}");
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

    #[test]
    fn unsupported_shells_are_accepted() {
        assert!(check(&ShellKind::Fish, "not checked").is_ok());
        assert!(check(&ShellKind::PosixSh, "not checked").is_ok());
    }
}

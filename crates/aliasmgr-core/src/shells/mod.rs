use crate::{
    error::AliasError,
    model::{AliasRecord, ManagedNameSet, ShellKind},
};

pub mod bash;
pub mod common;
pub mod powershell;
pub mod zsh;

/// Renders one alias through the same Shell adapter used by sync.
///
/// This is intentionally a thin dispatcher: all quoting, `{{args}}` handling,
/// restriction support, working-directory and environment rules stay in the
/// adapters. It is used for a per-alias preview and must never grow a second
/// rendering implementation.
pub fn render_alias(alias: &AliasRecord, shell: &ShellKind) -> Result<String, AliasError> {
    let names = ManagedNameSet {
        current: Vec::new(),
        retired: Vec::new(),
    };
    match shell {
        ShellKind::Bash => bash::render(alias, &names),
        ShellKind::Zsh => zsh::render(alias, &names),
        ShellKind::PowerShell5 | ShellKind::PowerShell7 => {
            powershell::render(alias, shell.clone(), &names)
        }
        ShellKind::Fish | ShellKind::PosixSh => Err(AliasError::ShellNotInstalled),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alias(name: &str) -> AliasRecord {
        AliasRecord {
            name: name.into(),
            executable: "git".into(),
            fixed_args: vec!["status".into()],
            shells: vec![ShellKind::Bash, ShellKind::PowerShell7],
            ..Default::default()
        }
    }

    #[test]
    fn per_alias_render_uses_the_shell_adapters_and_only_the_selected_name() {
        let value = alias("good");
        let bash = render_alias(&value, &ShellKind::Bash).unwrap();
        assert!(bash.contains("good"));
        assert!(!bash.contains("unrelated"));
        let powershell = render_alias(&value, &ShellKind::PowerShell7).unwrap();
        assert!(powershell.contains("good"));
        assert!(!powershell.contains("unrelated"));
        assert!(matches!(
            render_alias(&value, &ShellKind::Fish),
            Err(AliasError::ShellNotInstalled)
        ));
    }
}

use crate::{error::AliasError, model::{AliasRecord, ManagedNameSet, ShellKind}};

pub fn render(alias: &AliasRecord, shell: &ShellKind) -> Result<String, AliasError> {
    match shell { ShellKind::Bash => crate::shells::bash::render(alias, &ManagedNameSet { current: vec![], retired: vec![] }), ShellKind::Zsh => crate::shells::zsh::render(alias, &ManagedNameSet { current: vec![], retired: vec![] }), _ => Err(AliasError::ShellNotInstalled), }
}

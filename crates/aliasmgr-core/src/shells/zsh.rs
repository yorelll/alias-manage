use crate::{error::AliasError, model::{AliasRecord, ManagedNameSet}};
use super::{bash, common::quote_posix};

pub fn render(alias: &AliasRecord, names: &ManagedNameSet) -> Result<String, AliasError> { bash::render(alias, names) }
pub fn render_preemption(name: &str) -> String { format!("\nunalias {} 2>/dev/null\n", quote_posix(name)) }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn renders_zsh_function_with_preemption() {
        let alias = AliasRecord { name: "gs".into(), executable: "git".into(), fixed_args: vec!["status".into()], ..Default::default() };
        let text = render(&alias, &ManagedNameSet { current: vec![], retired: vec![] }).unwrap();
        assert!(text.contains("unalias 'gs'")); assert!(text.contains("gs()"));
    }
}

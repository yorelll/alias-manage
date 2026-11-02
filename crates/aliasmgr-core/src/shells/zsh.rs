use super::{bash, common::quote_posix};
use crate::{
    error::AliasError,
    model::{AliasRecord, ManagedNameSet},
};

pub fn render(alias: &AliasRecord, names: &ManagedNameSet) -> Result<String, AliasError> {
    bash::render(alias, names)
}
pub fn render_preemption(name: &str) -> String {
    format!("\nunalias {} 2>/dev/null\n", quote_posix(name))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn renders_zsh_function_with_preemption() {
        let alias = AliasRecord {
            name: "gs".into(),
            executable: "git".into(),
            fixed_args: vec!["status".into()],
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
        assert!(text.contains("unalias 'gs'"));
        assert!(text.contains("gs()"));
    }

    #[test]
    fn renders_restriction_guard_for_zsh_wrappers() {
        let alias = AliasRecord {
            name: "rma".into(),
            executable: "rm".into(),
            pass_args: false,
            restriction: Some(crate::model::PathRestriction {
                mode: crate::model::RestrictionMode::Whitelist,
                paths: vec!["/srv/project".into()],
            }),
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
        assert!(text.contains("local __aliasmgr_cwd"));
        assert!(text.contains("current directory is not in the whitelist"));
        assert!(!text.contains("alias rma="));
    }
}

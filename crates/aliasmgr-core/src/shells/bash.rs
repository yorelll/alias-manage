use crate::{error::AliasError, executor::render_argv, model::{AliasRecord, ManagedNameSet}};
use super::common::{is_simple_alias, quote_posix, render_cleanup};

pub fn render(alias: &AliasRecord, names: &ManagedNameSet) -> Result<String, AliasError> {
    let argv = render_argv(alias, &[])?;
    let command = argv.iter().map(|value| quote_posix(value)).collect::<Vec<_>>().join(" ");
    let mut output = render_cleanup(names);
    output.push_str(&render_preemption(&alias.name));
    if is_simple_alias(alias) { output.push_str(&format!("{}\n", crate::shells::common::managed_definition(&alias.name, &format!("alias {}={}", alias.name, quote_posix(&alias.executable))))); }
    else if alias.target_type == crate::model::TargetType::ChangeDirectory { output.push_str(&format!("{}\n", crate::shells::common::managed_definition(&alias.name, &format!("{}() {{ cd -- {}; }}", alias.name, quote_posix(alias.working_directory.as_deref().unwrap_or(&alias.executable)))))); }
    else { output.push_str(&format!("{}\n", crate::shells::common::managed_definition(&alias.name, &format!("{}() {{ command {} \"$@\"; }}", alias.name, command)))); }
    Ok(output)
}

pub fn render_preemption(name: &str) -> String { format!("\nunalias {} 2>/dev/null\n", quote_posix(name)) }

pub fn render_session_cleanup(name: &str, expected_fingerprint: &str) -> String {
    format!("if [ \"$(alias {} 2>/dev/null)\" = \"{}\" ]; then unalias {} 2>/dev/null; fi\n", quote_posix(name), expected_fingerprint.replace('"', "\\\""), quote_posix(name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AliasRecord, TargetType};

    #[test]
    fn renders_function_and_preemption() {
        let alias = AliasRecord { name: "cm".into(), executable: "python3".into(), fixed_args: vec!["script.py".into()], ..Default::default() };
        let text = render(&alias, &ManagedNameSet { current: vec![], retired: vec![] }).unwrap();
        assert!(text.contains("unalias 'cm'")); assert!(text.contains("cm()")); assert!(text.contains("\"$@\""));
    }

    #[test]
    fn renders_simple_alias_and_escaped_quote() {
        let alias = AliasRecord { name: "ll".into(), executable: "ls -l".into(), pass_args: false, ..Default::default() };
        let text = render(&alias, &ManagedNameSet { current: vec![], retired: vec![] }).unwrap();
        assert!(text.contains("ll()"));
        let value = AliasRecord { name: "q".into(), executable: "it's".into(), target_type: TargetType::NativeExecutable, ..Default::default() };
        assert!(render(&value, &ManagedNameSet { current: vec![], retired: vec![] }).unwrap().contains("'\\''"));
    }

    #[test]
    fn renders_tombstone_cleanup() {
        let names = ManagedNameSet { current: vec!["new".into()], retired: vec!["old".into()] };
        let text = render(&AliasRecord { name: "new".into(), executable: "tool".into(), ..Default::default() }, &names).unwrap();
        assert!(text.contains("unalias 'old'")); assert!(text.contains("unset -f 'old'"));
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
        database.record_override("old", &crate::model::ShellKind::Bash, Some("alias old='tool'"), true).unwrap();
        database.record_override("other", &crate::model::ShellKind::Bash, None, false).unwrap();
        let entries = database.overridden_definitions().unwrap();
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().any(|entry| entry["recoverable"] == true));
        assert!(entries.iter().any(|entry| entry["recoverable"] == false));
    }
}

use crate::{error::AliasError, executor::render_argv, model::{AliasRecord, ManagedNameSet, ShellKind, TargetType}};

pub fn quote(value: &str) -> String { format!("'{}'", value.replace('\'', "''")) }
pub fn is_simple_alias(alias: &AliasRecord) -> bool { alias.fixed_args.is_empty() && !alias.pass_args && alias.working_directory.is_none() && alias.environment.is_empty() && alias.target_type != TargetType::ChangeDirectory && !alias.executable.chars().any(|c| c.is_whitespace() || "'\"$;&|<>".contains(c)) }
pub fn render_preemption(name: &str) -> String { format!("Remove-Item -LiteralPath Alias:\\{} -Force -ErrorAction SilentlyContinue\nRemove-Item -LiteralPath Function:\\{} -Force -ErrorAction SilentlyContinue\n", name, name) }
pub fn render_cleanup(names: &ManagedNameSet) -> String { names.current.iter().chain(names.retired.iter()).map(|name| format!("Remove-Item -LiteralPath Alias:\\{} -Force -ErrorAction SilentlyContinue\nRemove-Item -LiteralPath Function:\\{} -Force -ErrorAction SilentlyContinue\n", name, name)).collect() }

pub fn render(alias: &AliasRecord, shell: ShellKind, names: &ManagedNameSet) -> Result<String, AliasError> {
    if !matches!(shell, ShellKind::PowerShell5 | ShellKind::PowerShell7) { return Err(AliasError::ShellNotInstalled); }
    let mut output = render_cleanup(names);
    output.push_str(&render_preemption(&alias.name));
    if is_simple_alias(alias) { output.push_str(&format!("Set-Alias -Name {} -Value {} -Scope Global -Force\n", alias.name, quote(&alias.executable))); return Ok(output); }
    let argv = render_argv(alias, &[])?;
    let command = argv.iter().map(|value| quote(value)).collect::<Vec<_>>().join(" ");
    output.push_str(&format!("function global:{} {{ & {} @args }}\n", alias.name, command));
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn names() -> ManagedNameSet { ManagedNameSet { current: vec![], retired: vec![] } }
    #[test]
    fn renders_simple_set_alias_and_preemption() {
        let alias = AliasRecord { name: "ll".into(), executable: "Get-ChildItem".into(), pass_args: false, ..Default::default() };
        let text = render(&alias, ShellKind::PowerShell7, &names()).unwrap();
        assert!(text.contains("Set-Alias -Name ll")); assert!(text.contains("Alias:\\ll")); assert!(text.contains("Function:\\ll"));
    }
    #[test]
    fn renders_function_with_fixed_args_and_array_passthrough() {
        let alias = AliasRecord { name: "gs".into(), executable: "git.exe".into(), fixed_args: vec!["status".into()], ..Default::default() };
        let text = render(&alias, ShellKind::PowerShell5, &names()).unwrap();
        assert!(text.contains("function global:gs")); assert!(text.contains("@args")); assert!(text.contains("'git.exe'"));
    }
    #[test]
    fn renders_cleanup_for_tombstones_and_escapes_literals() {
        let alias = AliasRecord { name: "q".into(), executable: "it's".into(), ..Default::default() };
        let names = ManagedNameSet { current: vec!["q".into()], retired: vec!["old".into()] };
        let text = render(&alias, ShellKind::PowerShell7, &names).unwrap();
        assert!(text.contains("Alias:\\old")); assert!(text.contains("it''s"));
    }
}

use crate::{error::AliasError, model::{AliasRecord, TargetType}};
use std::{collections::BTreeMap, path::Path};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSpec {
    pub argv: Vec<String>,
    pub working_directory: Option<String>,
    pub environment: BTreeMap<String, String>,
}

pub trait Executor {
    fn target_type(&self) -> TargetType;
    fn validate(&self, alias: &AliasRecord) -> Result<(), AliasError>;
    fn render_argv(&self, alias: &AliasRecord, args: &[String]) -> Result<CommandSpec, AliasError>;
}

pub struct StructuredExecutor;
pub struct BatchExecutor;

impl Executor for StructuredExecutor {
    fn target_type(&self) -> TargetType { TargetType::NativeExecutable }
    fn validate(&self, alias: &AliasRecord) -> Result<(), AliasError> {
        if alias.executable.trim().is_empty() { return Err(AliasError::TargetMissing(alias.executable.clone())); }
        if !Path::new(&alias.executable).exists() && alias.executable.contains('/') || alias.executable.contains('\\') { return Err(AliasError::TargetMissing(alias.executable.clone())); }
        Ok(())
    }
    fn render_argv(&self, alias: &AliasRecord, args: &[String]) -> Result<CommandSpec, AliasError> {
        self.validate(alias)?;
        if alias.target_type == TargetType::RawShellCommand || alias.advanced_shell_mode { return Err(AliasError::AdvancedModeUnsupported); }
        Ok(CommandSpec { argv: expand_args(alias, args), working_directory: alias.working_directory.clone(), environment: alias.environment.clone() })
    }
}

impl Executor for BatchExecutor {
    fn target_type(&self) -> TargetType { TargetType::Batch }
    fn validate(&self, alias: &AliasRecord) -> Result<(), AliasError> {
        if alias.executable.trim().is_empty() { return Err(AliasError::TargetMissing(alias.executable.clone())); }
        for arg in &alias.fixed_args { reject_batch_metacharacters(arg)?; }
        Ok(())
    }
    fn render_argv(&self, alias: &AliasRecord, args: &[String]) -> Result<CommandSpec, AliasError> {
        self.validate(alias)?;
        for arg in args { reject_batch_metacharacters(arg)?; }
        Ok(CommandSpec { argv: expand_args(alias, args), working_directory: alias.working_directory.clone(), environment: alias.environment.clone() })
    }
}

pub fn executor_for(alias: &AliasRecord) -> Result<Box<dyn Executor>, AliasError> {
    if alias.target_type == TargetType::Batch { Ok(Box::new(BatchExecutor)) } else { Ok(Box::new(StructuredExecutor)) }
}

pub fn render_argv(alias: &AliasRecord, args: &[String]) -> Result<Vec<String>, AliasError> {
    Ok(executor_for(alias)?.render_argv(alias, args)?.argv)
}

fn expand_args(alias: &AliasRecord, args: &[String]) -> Vec<String> {
    let mut out = vec![alias.executable.clone()];
    for item in &alias.fixed_args {
        if item == "{{args}}" { out.extend(args.iter().cloned()); }
        else if item == "{{{{args}}}}" { out.push("{{args}}".into()); }
        else { out.push(item.clone()); }
    }
    if alias.pass_args && !alias.fixed_args.iter().any(|item| item == "{{args}}") { out.extend(args.iter().cloned()); }
    out
}

fn reject_batch_metacharacters(value: &str) -> Result<(), AliasError> {
    if value.chars().any(|character| "%!&|^<>".contains(character)) { return Err(AliasError::InvalidArgTemplate); }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::AliasRecord;

    fn alias(target_type: TargetType) -> AliasRecord {
        AliasRecord { target_type, executable: "tool".into(), ..Default::default() }
    }

    #[test]
    fn expands_native_arguments_without_joining_strings() {
        let mut value = alias(TargetType::NativeExecutable);
        value.fixed_args = vec!["build".into(), "{{args}}".into(), "--verbose".into()];
        assert_eq!(render_argv(&value, &["a b".into(), "".into()]).unwrap(), vec!["tool", "build", "a b", "", "--verbose"]);
    }

    #[test]
    fn selects_script_and_jar_as_structured_commands() {
        for target in [TargetType::PythonScript, TargetType::PowerShellScript, TargetType::JavaJar, TargetType::ChangeDirectory] {
            assert!(matches!(executor_for(&alias(target)).unwrap().target_type(), TargetType::NativeExecutable));
        }
    }

    #[test]
    fn batch_executor_rejects_command_metacharacters() {
        let value = AliasRecord { target_type: TargetType::Batch, executable: "tool.bat".into(), fixed_args: vec!["bad&arg".into()], ..Default::default() };
        assert!(matches!(render_argv(&value, &[]), Err(AliasError::InvalidArgTemplate)));
    }

    #[test]
    fn carries_working_directory_and_environment_without_string_joining() {
        let mut value = alias(TargetType::NativeExecutable);
        value.working_directory = Some("/tmp/work dir".into());
        value.environment.insert("MODE".into(), "test value".into());
        let spec = executor_for(&value).unwrap().render_argv(&value, &[]).unwrap();
        assert_eq!(spec.working_directory.as_deref(), Some("/tmp/work dir"));
        assert_eq!(spec.environment["MODE"], "test value");
    }

    #[test]
    fn reports_missing_path_targets() {
        let value = AliasRecord { executable: "/definitely/missing/aliasmgr-target".into(), ..Default::default() };
        assert!(matches!(executor_for(&value).unwrap().render_argv(&value, &[]), Err(AliasError::TargetMissing(_))));
    }
}

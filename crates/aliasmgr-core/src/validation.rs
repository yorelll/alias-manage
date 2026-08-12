use crate::{error::AliasError, model::{AliasRecord, ShellKind, TargetType}};

pub fn validate_alias_name(name: &str) -> Result<(), AliasError> {
    if name.is_empty() || name.chars().count() > 64 {
        return Err(AliasError::InvalidAliasName);
    }
    for (index, character) in name.chars().enumerate() {
        let valid = if index == 0 {
            character.is_ascii_alphabetic() || character == '_'
        } else {
            character.is_ascii_alphanumeric() || character == '_' || character == '-'
        };
        if !valid { return Err(AliasError::InvalidAliasName); }
    }
    Ok(())
}

pub fn validate_arg_template(alias: &AliasRecord) -> Result<(), AliasError> {
    let placeholder_count = alias.fixed_args.iter().filter(|arg| arg.as_str() == "{{args}}").count();
    if placeholder_count > 1 || (placeholder_count == 1 && !alias.pass_args) {
        return Err(AliasError::InvalidArgTemplate);
    }
    Ok(())
}

pub fn validate_powershell_name(name: &str) -> Result<(), AliasError> {
    const RESERVED: &[&str] = &["ls", "cp", "mv", "rm", "cat", "gc"];
    if RESERVED.iter().any(|reserved| reserved.eq_ignore_ascii_case(name)) {
        return Err(AliasError::NameReserved);
    }
    Ok(())
}

pub fn validate_alias(alias: &AliasRecord) -> Result<(), AliasError> {
    validate_alias_name(&alias.name)?;
    if alias.executable.trim().is_empty() || alias.shells.is_empty() {
        return Err(AliasError::InvalidAliasName);
    }
    if alias.advanced_shell_mode || alias.target_type == TargetType::RawShellCommand {
        return Err(AliasError::AdvancedModeUnsupported);
    }
    validate_arg_template(alias)?;
    if alias.shells.iter().any(|shell| matches!(shell, ShellKind::PowerShell5 | ShellKind::PowerShell7)) {
        validate_powershell_name(&alias.name)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alias(name: &str) -> AliasRecord {
        let mut value = AliasRecord::default();
        value.name = name.into();
        value.executable = "tool".into();
        value
    }

    #[test]
    fn accepts_valid_names_and_rejects_invalid_names() {
        for name in ["cm", "copy-mv", "build_cam", "g1"] { assert!(validate_alias_name(name).is_ok()); }
        for name in ["my alias", "a=b", "hello;world", "$cmd", ""] { assert!(validate_alias_name(name).is_err()); }
        assert!(validate_alias_name(&"a".repeat(65)).is_err());
    }

    #[test]
    fn validates_argument_placeholders() {
        let mut value = alias("cm");
        value.fixed_args = vec!["build".into(), "{{args}}".into()];
        assert!(validate_alias(&value).is_ok());
        value.pass_args = false;
        assert!(matches!(validate_alias(&value), Err(AliasError::InvalidArgTemplate)));
        value.pass_args = true;
        value.fixed_args.push("{{args}}".into());
        assert!(matches!(validate_alias(&value), Err(AliasError::InvalidArgTemplate)));
        value.fixed_args = vec!["{{{{args}}}}".into()];
        assert!(validate_alias(&value).is_ok());
    }

    #[test]
    fn rejects_advanced_mode_and_reserved_powershell_names() {
        let mut value = alias("cm");
        value.advanced_shell_mode = true;
        assert!(matches!(validate_alias(&value), Err(AliasError::AdvancedModeUnsupported)));
        value.advanced_shell_mode = false;
        value.name = "ls".into();
        value.shells = vec![ShellKind::PowerShell7];
        assert!(matches!(validate_alias(&value), Err(AliasError::NameReserved)));
    }
}

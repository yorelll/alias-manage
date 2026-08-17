use aliasmgr_core::error::AliasError;

pub fn exit_code(error: &AliasError) -> i32 {
    match error {
        AliasError::AliasConflict(_) | AliasError::ExactNameConflict(_) | AliasError::CaseFoldConflict(_) | AliasError::NameReserved => 3,
        AliasError::TargetMissing(_) | AliasError::UnsafePath => 6,
        AliasError::InvalidAliasName | AliasError::InvalidArgTemplate | AliasError::AdvancedModeUnsupported => 5,
        AliasError::PermissionDenied => 7,
        AliasError::LockTimeout => 8,
        AliasError::SchemaTooNew | AliasError::ShellNotInstalled => 12,
        AliasError::ChecksumMismatch | AliasError::Database(_) | AliasError::Serialization(_) | AliasError::Io(_) | AliasError::UnreliableFilesystem => 13,
        AliasError::Config(_) => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn maps_stable_core_error_categories() {
        assert_eq!(exit_code(&AliasError::NameReserved), 3);
        assert_eq!(exit_code(&AliasError::ExactNameConflict("cm".into())), 3);
        assert_eq!(exit_code(&AliasError::CaseFoldConflict("Build".into())), 3);
        assert_eq!(exit_code(&AliasError::TargetMissing("x".into())), 6);
        assert_eq!(exit_code(&AliasError::InvalidArgTemplate), 5);
        assert_eq!(exit_code(&AliasError::ChecksumMismatch), 13);
        assert_eq!(exit_code(&AliasError::UnreliableFilesystem), 13);
        assert_eq!(exit_code(&AliasError::PermissionDenied), 7);
        assert_eq!(exit_code(&AliasError::LockTimeout), 8);
        assert_eq!(exit_code(&AliasError::SchemaTooNew), 12);
    }
}

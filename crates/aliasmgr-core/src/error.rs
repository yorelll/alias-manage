use thiserror::Error;

#[derive(Debug, Error)]
pub enum AliasError {
    #[error("invalid alias name")]
    InvalidAliasName,
    #[error("invalid argument template")]
    InvalidArgTemplate,
    #[error("advanced shell mode is unsupported")]
    AdvancedModeUnsupported,
    #[error("alias conflict: {0}")]
    AliasConflict(String),
    #[error("name is reserved")]
    NameReserved,
    #[error("target is missing: {0}")]
    TargetMissing(String),
    #[error("database schema is newer than this program")]
    SchemaTooNew,
    #[error("lock acquisition timed out")]
    LockTimeout,
    #[error("permission denied")]
    PermissionDenied,
    #[error("unsafe path")]
    UnsafePath,
    #[error("configuration error: {0}")]
    Config(String),
    #[error("shell is not installed")]
    ShellNotInstalled,
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

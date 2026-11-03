pub mod config;
pub mod detection;
pub mod doctor;
pub mod error;
pub mod executor;
pub mod fsutil;
pub mod integration;
pub mod lock;
pub mod migrations;
pub mod model;
pub mod rotation;
pub mod search;
pub mod service;
pub mod shell_setup;
pub mod shells;
pub mod storage;
pub mod sync;
pub mod syntax;
pub mod transfer;
pub mod uninstall;
pub mod validation;

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Serializes core tests that mutate process-wide environment variables.
/// Module-local locks are not enough because config and integration tests both
/// change HOME/USERPROFILE and run in the same test binary.
#[cfg(test)]
pub(crate) static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
mod tests {
    #[test]
    fn workspace_builds() {
        assert_eq!(super::version(), "0.1.0");
    }
}

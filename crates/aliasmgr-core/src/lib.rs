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

#[cfg(test)]
mod tests {
    #[test]
    fn workspace_builds() {
        assert_eq!(super::version(), "0.1.0");
    }
}

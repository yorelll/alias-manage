pub mod error;
pub mod model;
pub mod validation;
pub mod executor;
pub mod shell;
pub mod storage;
pub mod search;
pub mod config;
pub mod migrations;

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

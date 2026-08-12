pub mod error;
pub mod migrations;
pub mod search;
pub mod config;
pub mod model;
pub mod storage;
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

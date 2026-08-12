pub mod error;
pub mod model;
pub mod validation;
pub mod storage;
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

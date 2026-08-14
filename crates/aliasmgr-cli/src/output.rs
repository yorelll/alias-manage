use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct AliasRow { pub name: String, pub target: String, pub enabled: bool }

pub fn table(rows: &[AliasRow]) -> String {
    let mut output = String::from("NAME\tTARGET\tENABLED\n");
    for row in rows { output.push_str(&format!("{}\t{}\t{}\n", row.name, row.target, row.enabled)); }
    output
}

#[allow(dead_code)]
pub fn json<T: Serialize>(value: &T) -> Result<String, serde_json::Error> { serde_json::to_string_pretty(value) }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn output_columns_and_json_fields_are_stable() {
        let rows = vec![AliasRow { name: "gs".into(), target: "git".into(), enabled: true }];
        assert_eq!(table(&rows), "NAME\tTARGET\tENABLED\ngs\tgit\ttrue\n");
        assert!(json(&rows).unwrap().contains("\"name\""));
    }
}

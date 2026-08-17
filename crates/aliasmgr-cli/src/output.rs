use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct AliasRow {
    pub name: String,
    pub description: String,
    pub target: String,
    pub target_type: String,
    pub fixed_args: Vec<String>,
    pub pass_args: bool,
    pub working_directory: Option<String>,
    pub environment: std::collections::BTreeMap<String, String>,
    pub shells: Vec<String>,
    pub enabled: bool,
    pub tags: Vec<String>,
    pub revision: i64,
}

pub fn row(alias: &aliasmgr_core::model::AliasRecord) -> AliasRow {
    AliasRow {
        name: alias.name.clone(),
        description: alias.description.clone(),
        target: alias.executable.clone(),
        target_type: serde_json::to_value(&alias.target_type).unwrap().as_str().unwrap().to_owned(),
        fixed_args: alias.fixed_args.clone(),
        pass_args: alias.pass_args,
        working_directory: alias.working_directory.clone(),
        environment: alias.environment.clone(),
        shells: alias.shells.iter().map(|shell| serde_json::to_value(shell).unwrap().as_str().unwrap().to_owned()).collect(),
        enabled: alias.enabled,
        tags: alias.tags.clone(),
        revision: alias.revision,
    }
}

pub fn rows(aliases: &[aliasmgr_core::model::AliasRecord]) -> Vec<AliasRow> { aliases.iter().map(row).collect() }

pub fn alias_rows(aliases: &[aliasmgr_core::model::AliasRecord], format: crate::cli::OutputFormat) -> String {
    let rows = rows(aliases);
    match format {
        crate::cli::OutputFormat::Table => table(&rows),
        crate::cli::OutputFormat::Json => json(&rows).unwrap_or_else(|error| format!("serialization error: {error}")),
    }
}

pub fn table(rows: &[AliasRow]) -> String {
    let mut output = String::from("NAME\tTARGET\tENABLED\n");
    for row in rows { output.push_str(&format!("{}\t{}\t{}\n", row.name, row.target, row.enabled)); }
    output
}

pub fn json<T: Serialize>(value: &T) -> Result<String, serde_json::Error> { serde_json::to_string_pretty(value) }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_columns_and_json_fields_are_stable() {
        let rows = vec![AliasRow { name: "gs".into(), description: "".into(), target: "git".into(), target_type: "native_executable".into(), fixed_args: vec![], pass_args: true, working_directory: None, environment: Default::default(), shells: vec!["bash".into()], enabled: true, tags: vec![], revision: 1 }];
        assert_eq!(table(&rows), "NAME\tTARGET\tENABLED\ngs\tgit\ttrue\n");
        assert!(json(&rows).unwrap().contains("\"name\""));
    }
}

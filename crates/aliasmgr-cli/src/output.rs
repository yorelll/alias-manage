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
    pub restriction: Option<aliasmgr_core::model::PathRestriction>,
    pub revision: i64,
}

pub fn row(alias: &aliasmgr_core::model::AliasRecord) -> AliasRow {
    AliasRow {
        name: alias.name.clone(),
        description: alias.description.clone(),
        target: alias.executable.clone(),
        target_type: serde_json::to_value(&alias.target_type)
            .unwrap()
            .as_str()
            .unwrap()
            .to_owned(),
        fixed_args: alias.fixed_args.clone(),
        pass_args: alias.pass_args,
        working_directory: alias.working_directory.clone(),
        environment: alias.environment.clone(),
        shells: alias
            .shells
            .iter()
            .map(|shell| {
                serde_json::to_value(shell)
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_owned()
            })
            .collect(),
        enabled: alias.enabled,
        tags: alias.tags.clone(),
        restriction: alias.restriction.clone(),
        revision: alias.revision,
    }
}

pub fn rows(aliases: &[aliasmgr_core::model::AliasRecord]) -> Vec<AliasRow> {
    aliases.iter().map(row).collect()
}

pub fn alias_rows(
    aliases: &[aliasmgr_core::model::AliasRecord],
    format: crate::cli::OutputFormat,
) -> String {
    let rows = rows(aliases);
    match format {
        crate::cli::OutputFormat::Table => table(&rows),
        crate::cli::OutputFormat::Json => {
            json(&rows).unwrap_or_else(|error| format!("serialization error: {error}"))
        }
    }
}

/// Table cells must stay on one line: control characters (tabs, newlines,
/// escape sequences) become spaces so a crafted description or tag cannot
/// break the column layout or inject terminal control sequences.
fn sanitize_cell(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect()
}

pub fn table(rows: &[AliasRow]) -> String {
    let mut output = String::from("NAME\tTARGET\tDESCRIPTION\tTAGS\tENABLED\n");
    for row in rows {
        output.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\n",
            sanitize_cell(&row.name),
            sanitize_cell(&row.target),
            sanitize_cell(&row.description),
            sanitize_cell(&row.tags.join(", ")),
            row.enabled
        ));
    }
    output
}

pub fn json<T: Serialize>(value: &T) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_columns_and_json_fields_are_stable() {
        let rows = vec![AliasRow {
            name: "gs".into(),
            description: "".into(),
            target: "git".into(),
            target_type: "native_executable".into(),
            fixed_args: vec![],
            pass_args: true,
            working_directory: None,
            environment: Default::default(),
            shells: vec!["bash".into()],
            enabled: true,
            tags: vec![],
            restriction: None,
            revision: 1,
        }];
        assert_eq!(
            table(&rows),
            "NAME\tTARGET\tDESCRIPTION\tTAGS\tENABLED\ngs\tgit\t\t\ttrue\n"
        );
        assert!(json(&rows).unwrap().contains("\"name\""));
    }

    #[test]
    fn table_columns_show_description_and_tags_and_strip_control_characters() {
        let rows = vec![AliasRow {
            name: "gs".into(),
            description: "line one\nline\ttwo".into(),
            target: "git".into(),
            target_type: "native_executable".into(),
            fixed_args: vec![],
            pass_args: true,
            working_directory: None,
            environment: Default::default(),
            shells: vec!["bash".into()],
            enabled: true,
            tags: vec!["work".into(), "a\tb".into()],
            restriction: None,
            revision: 1,
        }];
        assert_eq!(
            table(&rows),
            "NAME\tTARGET\tDESCRIPTION\tTAGS\tENABLED\ngs\tgit\tline one line two\twork, a b\ttrue\n"
        );
    }
}

use crate::model::AliasRecord;

pub const EXACT_WEIGHT: i32 = 1000;
pub const PREFIX_WEIGHT: i32 = 800;
pub const SUBSTRING_WEIGHT: i32 = 600;
pub const FUZZY_WEIGHT: i32 = 400;
pub const TAG_WEIGHT: i32 = 350;
pub const FIELD_WEIGHT: i32 = 200;
pub const EDIT_DISTANCE_WEIGHT: i32 = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchField { All, Name, Target, Description, Tags, Shells, TargetType }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortField { Name, UpdatedAt, CreatedAt, TargetType, Enabled }

#[derive(Debug, Clone)]
pub struct SearchQuery {
    pub query: String,
    pub fuzzy: bool,
    pub field: SearchField,
    pub limit: usize,
    pub sort: SortField,
    pub descending: bool,
    pub tag_filter: Vec<String>,

impl Default for SearchQuery {
    fn default() -> Self {
        Self { query: String::new(), fuzzy: false, field: SearchField::All, limit: 50, sort: SortField::Name, descending: false, tag_filter: Vec::new() }
    }
}

#[derive(Debug, Clone)]
pub struct SearchResult { pub alias: AliasRecord, pub score: i32 }

pub fn score(alias: &AliasRecord, query: &str) -> i32 {
    score_field(alias, query, SearchField::All, false)
}

pub fn search(aliases: &[AliasRecord], query: &SearchQuery) -> Vec<SearchResult> {
    let mut results: Vec<_> = aliases.iter().filter_map(|alias| {
        let tags_match = query.tag_filter.iter().all(|wanted| alias.tags.iter().any(|tag| tag.eq_ignore_ascii_case(wanted)));
        if !tags_match { return None; }
        let score = score_field(alias, &query.query, query.field, query.fuzzy);
        (query.query.is_empty() || score > 0).then(|| SearchResult { alias: alias.clone(), score })
    }).collect();
    results.sort_by(|left, right| {
        right.score.cmp(&left.score)
            .then_with(|| left.alias.name.cmp(&right.alias.name))
            .then_with(|| right.alias.updated_at.cmp(&left.alias.updated_at))
    });
    if query.limit > 0 { results.truncate(query.limit); }
    results
}

fn score_field(alias: &AliasRecord, query: &str, field: SearchField, fuzzy: bool) -> i32 {
    let query = query.to_lowercase();
    if query.is_empty() { return 0; }
    let name = alias.name.to_lowercase();
    let name_score = if name == query { EXACT_WEIGHT } else if name.starts_with(&query) { PREFIX_WEIGHT } else if name.contains(&query) { SUBSTRING_WEIGHT } else if fuzzy && is_subsequence(&query, &name) { FUZZY_WEIGHT } else { 0 };
    if matches!(field, SearchField::Name) { return name_score; }
    if matches!(field, SearchField::All) && name_score > 0 { return name_score; }
    let fields = match field {
        SearchField::Target | SearchField::All => vec![alias.executable.to_lowercase()],
        SearchField::Description => vec![alias.description.to_lowercase()],
        SearchField::Tags => alias.tags.iter().map(|tag| tag.to_lowercase()).collect(),
        SearchField::Shells => alias.shells.iter().map(|shell| format!("{shell:?}").to_lowercase()).collect(),
        SearchField::TargetType => vec![format!("{:?}", alias.target_type).to_lowercase()],
        SearchField::Name => Vec::new(),
    };
    fields.into_iter().map(|value| if value == query { TAG_WEIGHT } else if value.contains(&query) { FIELD_WEIGHT } else { 0 }).max().unwrap_or(0)
}

fn is_subsequence(query: &str, value: &str) -> bool {
    let mut chars = value.chars();
    query.chars().all(|wanted| chars.by_ref().any(|current| current == wanted))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::AliasRecord;

    fn alias(name: &str, executable: &str) -> AliasRecord {
        AliasRecord { name: name.into(), executable: executable.into(), ..Default::default() }
    }

    #[test]
    fn scores_exact_prefix_substring_and_fuzzy_name_matches() {
        assert_eq!(score(&alias("cm", "tool"), "cm"), EXACT_WEIGHT);
        assert_eq!(score(&alias("cm-build", "tool"), "cm"), PREFIX_WEIGHT);
        assert_eq!(score(&alias("build-cm", "tool"), "cm"), SUBSTRING_WEIGHT);
        let query = SearchQuery { query: "cb".into(), fuzzy: true, ..Default::default() };
        assert_eq!(search(&[alias("copy-build", "tool")], &query)[0].score, FUZZY_WEIGHT);
    }

    #[test]
    fn searches_target_and_applies_stable_limit() {
        let query = SearchQuery { query: "python".into(), field: SearchField::Target, limit: 1, ..Default::default() };
        let results = search(&[alias("zeta", "python3"), alias("alpha", "python3")], &query);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].alias.name, "alpha");
    }

    #[test]
    fn tag_filter_requires_all_requested_tags() {
        let mut both = alias("both", "tool");
        both.tags = vec!["git".into(), "work".into()];
        let mut one = alias("one", "tool");
        one.tags = vec!["git".into()];
        let query = SearchQuery { tag_filter: vec!["git".into(), "work".into()], ..Default::default() };
        let results = search(&[both, one], &query);
        assert_eq!(results.iter().map(|result| result.alias.name.as_str()).collect::<Vec<_>>(), vec!["both"]);
    }
}

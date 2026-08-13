use crate::error::AliasError;
use crate::model::{AliasRecord, ManagedNameSet};
use std::path::Path;

pub const START_MARKER: &str = "# >>> Alias Manager >>>";
pub const END_MARKER: &str = "# <<< Alias Manager <<<";

pub fn quote_posix(value: &str) -> String { format!("'{}'", value.replace('\'', "'\\''")) }

pub fn render_loader(generated_path: &Path) -> String {
    format!("{START_MARKER}\n# managed block, do not edit by hand\n__aliasmgr_generated_file={}\n[ -r \"$__aliasmgr_generated_file\" ] && . \"$__aliasmgr_generated_file\"\nunset -v __aliasmgr_generated_file\n{END_MARKER}\n", quote_posix(&generated_path.to_string_lossy()))
}

pub fn install_loader(content: &str, loader: &str) -> Result<String, AliasError> {
    if content.matches(START_MARKER).count() > 1 || content.matches(END_MARKER).count() > 1 { return Err(AliasError::UnsafePath); }
    if content.contains(START_MARKER) != content.contains(END_MARKER) { return Err(AliasError::UnsafePath); }
    if content.contains(START_MARKER) { return Ok(content.to_string()); }
    let separator = if content.is_empty() || content.ends_with('\n') { "" } else { "\n" };
    Ok(format!("{content}{separator}{loader}"))
}

pub fn remove_loader(content: &str) -> Result<String, AliasError> {
    let start = content.find(START_MARKER);
    let end = content.find(END_MARKER).map(|index| index + END_MARKER.len());
    match (start, end) {
        (None, None) => Ok(content.to_string()),
        (Some(start), Some(end)) => {
            let end = if content[end..].starts_with("\r\n") { end + 2 } else if content[end..].starts_with('\n') { end + 1 } else { end };
            let mut result = String::with_capacity(content.len()); result.push_str(&content[..start]); result.push_str(&content[end..]); Ok(result)
        }
        _ => Err(AliasError::UnsafePath),
    }
}

pub fn managed_names(names: &ManagedNameSet) -> Vec<String> {
    names.current.iter().chain(names.retired.iter()).cloned().collect()
}

pub fn render_cleanup(names: &ManagedNameSet) -> String {
    managed_names(names).iter().map(|name| format!("unalias {} 2>/dev/null\nunset -f {} 2>/dev/null\n", quote_posix(name), quote_posix(name))).collect()
}

pub fn is_simple_alias(alias: &AliasRecord) -> bool {
    alias.fixed_args.is_empty() && !alias.pass_args && alias.working_directory.is_none() && alias.environment.is_empty() && alias.target_type != crate::model::TargetType::ChangeDirectory && !alias.executable.chars().any(|c| c.is_whitespace() || "'\"$;&|<>".contains(c))
}

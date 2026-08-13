use crate::error::AliasError;
use crate::model::{AliasRecord, ManagedNameSet};
use std::{path::Path, fs};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoaderPosition { Missing, AtEnd, NotAtEnd }

pub fn loader_position(content: &str) -> Result<LoaderPosition, AliasError> {
    let start = content.matches(START_MARKER).count(); let end = content.matches(END_MARKER).count();
    if start > 1 || end > 1 || start != end { return Err(AliasError::UnsafePath); }
    if start == 0 { return Ok(LoaderPosition::Missing); }
    let end_index = content.rfind(END_MARKER).unwrap() + END_MARKER.len();
    Ok(if content[end_index..].trim().is_empty() { LoaderPosition::AtEnd } else { LoaderPosition::NotAtEnd })
}

pub fn write_loader_file(path: &Path, content: &str, loader: &str) -> Result<(), AliasError> {
    if path.exists() { let backup_dir = path.parent().unwrap_or_else(|| Path::new(".")).join("backups/rc"); fs::create_dir_all(&backup_dir)?; let backup = backup_dir.join(format!("rc-{}.bak", chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default())); fs::copy(path, backup)?; }
    let original = if path.exists() { fs::read_to_string(path)? } else { String::new() };
    fs::write(path, install_loader(&original, loader)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn loader_install_is_idempotent_and_appends() {
        let loader = render_loader(Path::new("/tmp/generated.sh"));
        let once = install_loader("user\n", &loader).unwrap();
        assert_eq!(install_loader(&once, &loader).unwrap(), once);
        assert_eq!(loader_position(&once).unwrap(), LoaderPosition::AtEnd);
    }
    #[test]
    fn loader_removal_preserves_user_content_and_rejects_unpaired_markers() {
        let loader = render_loader(Path::new("/tmp/generated.sh"));
        let content = install_loader("before\n", &loader).unwrap() + "after\n";
        assert!(remove_loader(&content).unwrap().contains("before"));
        assert!(remove_loader("# >>> Alias Manager >>>\n").is_err());
    }
}

// keep filesystem support in the module boundary for future atomic replacement tests
pub fn _filesystem_exists(path: &Path) -> bool { path.exists() }

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

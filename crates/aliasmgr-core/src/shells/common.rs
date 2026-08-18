#![allow(clippy::items_after_test_module)]

use crate::error::AliasError;
use crate::model::{AliasRecord, ManagedNameSet};
use sha2::{Digest, Sha256};
use std::{fs, path::Path};

pub fn fingerprint_marker(name: &str) -> String { let mut digest = Sha256::new(); digest.update(name.as_bytes()); format!("# aliasmgr fingerprint:{:x}", digest.finalize()) }
pub fn managed_definition(name: &str, definition: &str) -> String { format!("{definition} {}", fingerprint_marker(name)) }
pub const START_MARKER: &str = "# >>> Alias Manager >>>";
pub const END_MARKER: &str = "# <<< Alias Manager <<<";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoaderPosition { Missing, AtEnd, NotAtEnd }
pub fn loader_position(content: &str) -> Result<LoaderPosition, AliasError> { let start = content.matches(START_MARKER).count(); let end = content.matches(END_MARKER).count(); if start > 1 || end > 1 || start != end { return Err(AliasError::UnsafePath); } if start == 0 { return Ok(LoaderPosition::Missing); } let end_index = content.rfind(END_MARKER).unwrap() + END_MARKER.len(); Ok(if content[end_index..].trim().is_empty() { LoaderPosition::AtEnd } else { LoaderPosition::NotAtEnd }) }

pub fn write_loader_file(path: &Path, _content: &str, loader: &str) -> Result<(), AliasError> { let original = if path.exists() { fs::read_to_string(path)? } else { String::new() }; write_profile_content(path, &install_loader(&original, loader)?) }
pub fn write_profile_content(path: &Path, content: &str) -> Result<(), AliasError> { if path.exists() { let backup_dir = path.parent().unwrap_or_else(|| Path::new(".")).join("backups/rc"); fs::create_dir_all(&backup_dir)?; let backup = backup_dir.join(format!("rc-{}.bak", chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default())); fs::copy(path, backup)?; } fs::write(path, content)?; Ok(()) }
pub fn _filesystem_exists(path: &Path) -> bool { path.exists() }

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn fingerprint_marker_is_stable_and_embedded_in_managed_definition() { assert_eq!(fingerprint_marker("cm"), fingerprint_marker("cm")); assert_ne!(fingerprint_marker("cm"), fingerprint_marker("other")); assert!(managed_definition("cm", "cm() { true; }").contains("aliasmgr fingerprint:")); }
    #[test] fn loader_install_is_idempotent_and_appends() { let loader = render_loader(Path::new("/tmp/generated.sh")); let once = install_loader("user\n", &loader).unwrap(); assert_eq!(install_loader(&once, &loader).unwrap(), once); assert_eq!(loader_position(&once).unwrap(), LoaderPosition::AtEnd); }
    #[test] fn loader_removal_preserves_user_content_and_rejects_unpaired_markers() { let loader = render_loader(Path::new("/tmp/generated.sh")); let content = install_loader("before\n", &loader).unwrap() + "after\n"; assert!(remove_loader(&content).unwrap().contains("before")); assert!(remove_loader("# >>> Alias Manager >>>\n").is_err()); }
    #[test] fn loader_install_preserves_crlf_line_endings() { let loader = render_loader(Path::new("C:\\generated.sh")); let installed = install_loader("before\r\n", &loader).unwrap(); assert!(installed.contains("before\r\n")); assert!(installed.contains("# >>> Alias Manager >>>\r\n")); assert!(installed.contains("# <<< Alias Manager <<<\r\n")); }
    #[cfg(unix)] #[test] fn writing_loader_through_symlink_preserves_link_and_updates_target() { use std::os::unix::fs::symlink; let root = std::env::temp_dir().join(format!("aliasmgr-loader-link-{}", std::process::id())); fs::create_dir_all(&root).unwrap(); let target = root.join("real.rc"); let link = root.join("profile.rc"); fs::write(&target, "before\n").unwrap(); symlink(&target, &link).unwrap(); let loader = render_loader(Path::new("/tmp/generated.sh")); write_loader_file(&link, "before\n", &loader).unwrap(); assert!(link.symlink_metadata().unwrap().file_type().is_symlink()); assert!(fs::read_to_string(&target).unwrap().contains(START_MARKER)); let _ = fs::remove_dir_all(root); }
}

pub fn quote_posix(value: &str) -> String { format!("'{}'", value.replace('\'', "'\\''")) }
pub fn render_loader(generated_path: &Path) -> String { format!("{START_MARKER}\n# managed block, do not edit by hand\n__aliasmgr_generated_file={}\n[ -r \"$__aliasmgr_generated_file\" ] && . \"$__aliasmgr_generated_file\"\nunset -v __aliasmgr_generated_file\n{END_MARKER}\n", quote_posix(&generated_path.to_string_lossy())) }
pub fn install_loader(content: &str, loader: &str) -> Result<String, AliasError> { if content.matches(START_MARKER).count() > 1 || content.matches(END_MARKER).count() > 1 { return Err(AliasError::UnsafePath); } if content.contains(START_MARKER) != content.contains(END_MARKER) { return Err(AliasError::UnsafePath); } if content.contains(START_MARKER) { return Ok(content.to_string()); } let newline = if content.contains("\r\n") { "\r\n" } else { "\n" }; let loader = loader.replace("\r\n", "\n").replace('\n', newline); let separator = if content.is_empty() || content.ends_with('\n') { "" } else { newline }; Ok(format!("{content}{separator}{loader}")) }
pub fn remove_loader(content: &str) -> Result<String, AliasError> { let start = content.find(START_MARKER); let end = content.find(END_MARKER).map(|index| index + END_MARKER.len()); match (start, end) { (None, None) => Ok(content.to_string()), (Some(start), Some(end)) => { let end = if content[end..].starts_with("\r\n") { end + 2 } else if content[end..].starts_with('\n') { end + 1 } else { end }; let mut result = String::with_capacity(content.len()); result.push_str(&content[..start]); result.push_str(&content[end..]); Ok(result) }, _ => Err(AliasError::UnsafePath) } }
pub fn managed_names(names: &ManagedNameSet) -> Vec<String> { names.current.iter().chain(names.retired.iter()).cloned().collect() }
pub fn render_cleanup(names: &ManagedNameSet) -> String { managed_names(names).iter().map(|name| format!("# aliasmgr cleanup:{}\nunalias {} 2>/dev/null\nunset -f {} 2>/dev/null\n", fingerprint_marker(name), quote_posix(name), quote_posix(name))).collect() }
pub fn is_simple_alias(alias: &AliasRecord) -> bool { alias.fixed_args.is_empty() && !alias.pass_args && alias.working_directory.is_none() && alias.environment.is_empty() && alias.target_type != crate::model::TargetType::ChangeDirectory && !alias.executable.chars().any(|c| c.is_whitespace() || "'\"$;&|<>".contains(c)) }

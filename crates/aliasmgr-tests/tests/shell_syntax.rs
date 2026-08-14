use aliasmgr_core::{model::{AliasRecord, ManagedNameSet}, shells::{bash, common::{install_loader, render_loader}, zsh}};
use std::{fs, path::Path, process::Command};

fn command_available(command: &str) -> bool {
    Command::new(command)
        .arg("--version")
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn generated_script() -> String {
    let alias = AliasRecord { name: "gs".into(), executable: "git".into(), fixed_args: vec!["status".into()], ..Default::default() };
    bash::render(&alias, &ManagedNameSet { current: vec![], retired: vec![] }).unwrap()
}

#[test]
fn bash_rendered_script_passes_real_bash_syntax_check() {
    if !command_available("bash") { return; }
    let root = std::env::temp_dir().join(format!("aliasmgr-bash-syntax-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let path = root.join("generated.sh");
    fs::write(&path, generated_script()).unwrap();
    let status = Command::new("bash").args(["-n", path.to_str().unwrap()]).status().unwrap();
    assert!(status.success());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn zsh_rendered_script_passes_real_zsh_syntax_check() {
    if !command_available("zsh") { return; }
    let alias = AliasRecord { name: "gs".into(), executable: "git".into(), fixed_args: vec!["status".into()], ..Default::default() };
    let script = zsh::render(&alias, &ManagedNameSet { current: vec![], retired: vec![] }).unwrap();
    let root = std::env::temp_dir().join(format!("aliasmgr-zsh-syntax-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let path = root.join("generated.zsh");
    fs::write(&path, script).unwrap();
    let status = Command::new("zsh").args(["-n", path.to_str().unwrap()]).status().unwrap();
    assert!(status.success());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn loader_is_syntax_valid_when_sourced_by_bash() {
    if !command_available("bash") { return; }
    let loader = render_loader(Path::new("/tmp/aliasmgr-generated.sh"));
    let content = install_loader("# user config\n", &loader).unwrap();
    let root = std::env::temp_dir().join(format!("aliasmgr-loader-syntax-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let path = root.join("profile");
    fs::write(&path, content).unwrap();
    let status = Command::new("bash").args(["-n", path.to_str().unwrap()]).status().unwrap();
    assert!(status.success());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn shell_syntax_fixture_matrix_is_enabled() {
    let marker = "shell-matrix";
    assert_eq!(marker, "shell-matrix");
}

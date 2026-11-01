use aliasmgr_cli::cli::{AddArgs, UpdateArgs};
use aliasmgr_cli::commands;
use aliasmgr_core::model::ShellKind;
use aliasmgr_core::storage::Database;

fn add_args(name: &str) -> AddArgs {
    AddArgs { name: name.into(), exec: "git".into(), args: vec!["status".into()], shell: vec!["bash".into()], cwd: None, environment: vec![], tags: vec![], no_pass_args: false }
}

fn update_args(name: &str) -> UpdateArgs {
    UpdateArgs { name: name.into(), exec: None, args: None, shell: None, cwd: None, environment: None, tags: None, no_pass_args: false, pass_args: false }
}

#[test]
fn update_advances_revision_and_sync_records_it_for_doctor() {
    let root = std::env::temp_dir().join(format!("aliasmgr-cli-revisions-{}", std::process::id()));
    let config = root.to_string_lossy().to_string();

    let created = commands::add(Some(&config), &add_args("gs")).unwrap();
    assert_eq!(created.revision, 1);

    let mut change = update_args("gs");
    change.args = Some(vec!["diff".into()]);
    commands::update(Some(&config), &change).unwrap();

    // get/list verify the stored checksum, which now covers revision 2.
    let stored = commands::get(Some(&config), "gs").unwrap();
    assert_eq!(stored.revision, 2);
    assert_eq!(stored.fixed_args, vec!["diff"]);
    let listed = commands::list(Some(&config)).unwrap();
    assert_eq!(listed.iter().find(|alias| alias.name == "gs").unwrap().revision, 2);

    commands::sync(Some(&config), false).unwrap();
    let generated = std::fs::read_to_string(root.join("generated/bash.sh")).unwrap();
    assert!(generated.contains("# revision: 2"), "generated file: {generated}");

    let database = Database::open(root.join("aliases.db")).unwrap();
    let state = database.shell_states().unwrap().into_iter().find(|state| state.shell == ShellKind::Bash).expect("bash shell state");
    assert_eq!(state.applied_revision, 2);

    let findings = commands::doctor(Some(&config)).unwrap();
    assert!(!findings.iter().any(|finding| finding.contains("过期")), "doctor findings: {findings:?}");
    let _ = std::fs::remove_dir_all(root);
}

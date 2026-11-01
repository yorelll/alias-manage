use aliasmgr_cli::cli::{AddArgs, UpdateArgs};
use aliasmgr_cli::commands;
use aliasmgr_core::model::{record_checksum, ShellKind};
use aliasmgr_core::storage::Database;

fn add_args(name: &str) -> AddArgs {
    AddArgs {
        name: name.into(),
        exec: "git".into(),
        args: vec!["status".into()],
        shell: vec!["bash".into()],
        cwd: None,
        environment: vec![],
        tags: vec![],
        no_pass_args: false,
        target_type: None,
        path_mode: None,
    }
}

fn update_args(name: &str) -> UpdateArgs {
    UpdateArgs {
        name: name.into(),
        exec: None,
        args: None,
        shell: None,
        cwd: None,
        environment: None,
        tags: None,
        no_pass_args: false,
        pass_args: false,
        target_type: None,
        path_mode: None,
    }
}

fn revision_of(content: &str) -> i64 {
    content
        .lines()
        .find_map(|line| line.strip_prefix("# revision: "))
        .and_then(|value| value.trim().parse().ok())
        .expect("generated file has a revision header")
}

fn bash_state(config: &str) -> aliasmgr_core::model::ShellState {
    Database::open(std::path::Path::new(config).join("aliases.db"))
        .unwrap()
        .shell_states()
        .unwrap()
        .into_iter()
        .find(|state| state.shell == ShellKind::Bash)
        .expect("bash shell state")
}

#[test]
fn update_returns_the_persisted_record_and_sync_records_it_for_doctor() {
    let root = std::env::temp_dir().join(format!("aliasmgr-cli-revisions-{}", std::process::id()));
    let config = root.to_string_lossy().to_string();

    let created = commands::add(Some(&config), &add_args("gs")).unwrap();
    assert_eq!(created.revision, 1);

    let mut change = update_args("gs");
    change.args = Some(vec!["diff".into()]);
    let updated = commands::update(Some(&config), &change).unwrap();
    assert_eq!(
        updated.revision, 2,
        "update must return the persisted revision"
    );
    assert_eq!(updated.record_checksum, record_checksum(&updated).unwrap());

    // get/list verify the stored checksum, which now covers revision 2.
    let stored = commands::get(Some(&config), "gs").unwrap();
    assert_eq!(stored.revision, updated.revision);
    assert_eq!(stored.record_checksum, updated.record_checksum);
    assert_eq!(stored.updated_at, updated.updated_at);
    assert_eq!(stored.fixed_args, vec!["diff"]);
    let listed = commands::list(Some(&config)).unwrap();
    assert_eq!(
        listed
            .iter()
            .find(|alias| alias.name == "gs")
            .unwrap()
            .revision,
        2
    );

    commands::sync(Some(&config), false).unwrap();
    let generated = std::fs::read_to_string(root.join("generated/bash.sh")).unwrap();
    assert!(
        generated.contains("# revision: 2"),
        "generated file: {generated}"
    );
    assert_eq!(bash_state(&config).applied_revision, 2);

    let findings = commands::doctor(Some(&config)).unwrap();
    assert!(
        !findings.iter().any(|finding| finding.contains("过期")),
        "doctor findings: {findings:?}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn deletions_keep_revisions_monotonic_and_doctor_detects_staleness() {
    let root = std::env::temp_dir().join(format!(
        "aliasmgr-cli-revision-delete-{}",
        std::process::id()
    ));
    let config = root.to_string_lossy().to_string();

    commands::add(Some(&config), &add_args("a")).unwrap();
    commands::add(Some(&config), &add_args("b")).unwrap();
    commands::sync(Some(&config), false).unwrap();
    let generated = std::fs::read_to_string(root.join("generated/bash.sh")).unwrap();
    assert_eq!(revision_of(&generated), 1);
    assert!(generated.contains("b() {"), "generated file: {generated}");
    let applied_before = bash_state(&config).applied_revision;

    let mut change = update_args("b");
    change.args = Some(vec!["diff".into()]);
    let updated = commands::update(Some(&config), &change).unwrap();
    assert_eq!(updated.revision, 2);

    assert!(commands::remove(Some(&config), "b").unwrap());
    let tombstone = Database::open(root.join("aliases.db"))
        .unwrap()
        .max_retired_revision()
        .unwrap();
    assert!(
        tombstone > updated.revision,
        "tombstone {tombstone} must advance past the deleted alias revision {}",
        updated.revision
    );

    let findings = commands::doctor(Some(&config)).unwrap();
    assert!(
        findings
            .iter()
            .any(|finding| finding.contains("过期") && finding.contains("Bash")),
        "delete must leave bash stale: {findings:?}"
    );

    commands::sync(Some(&config), false).unwrap();
    let generated = std::fs::read_to_string(root.join("generated/bash.sh")).unwrap();
    let revision = revision_of(&generated);
    assert_eq!(
        revision, tombstone,
        "sync must adopt the tombstone revision"
    );
    assert!(
        revision > applied_before,
        "revision {revision} must advance past applied {applied_before}"
    );
    assert!(
        !generated.contains("b() {"),
        "removed alias still generated: {generated}"
    );
    assert!(generated.contains("# retired: b"));
    assert_eq!(bash_state(&config).applied_revision, revision);

    let findings = commands::doctor(Some(&config)).unwrap();
    assert!(
        !findings.iter().any(|finding| finding.contains("过期")),
        "doctor findings after sync: {findings:?}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn rename_retires_the_old_name_for_every_bound_shell() {
    let root = std::env::temp_dir().join(format!("aliasmgr-cli-rename-{}", std::process::id()));
    let config = root.to_string_lossy().to_string();

    let mut add = add_args("x");
    add.shell = vec!["bash".into(), "zsh".into()];
    commands::add(Some(&config), &add).unwrap();
    commands::sync(Some(&config), false).unwrap();
    for file in ["bash.sh", "zsh.sh"] {
        let content = std::fs::read_to_string(root.join("generated").join(file)).unwrap();
        assert!(content.contains("x() {"), "{file} must define x: {content}");
    }

    assert!(commands::rename(Some(&config), "x", "y").unwrap());
    commands::sync(Some(&config), false).unwrap();
    for file in ["bash.sh", "zsh.sh"] {
        let content = std::fs::read_to_string(root.join("generated").join(file)).unwrap();
        assert!(
            !content.contains("x() {"),
            "{file} still defines the old name: {content}"
        );
        assert!(
            content.contains("# retired: x"),
            "{file} misses the tombstone: {content}"
        );
        assert!(
            content.contains("unalias 'x'"),
            "{file} misses the cleanup: {content}"
        );
        assert!(
            content.contains("y() {"),
            "{file} misses the new name: {content}"
        );
        assert!(
            content.contains("# managed: y"),
            "{file} misses the managed line: {content}"
        );
    }
    let _ = std::fs::remove_dir_all(root);
}

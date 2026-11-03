use std::{
    path::{Path, PathBuf},
    process::{Command, Output},
};

fn temp_root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "aliasmgr-messages-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or_default()
    ))
}

fn run(config: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_aliasmgr"))
        .arg("--config-dir")
        .arg(config)
        .args(args)
        .output()
        .unwrap()
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn assert_message(output: Output, expected: &str) {
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(stdout(&output), expected);
}

#[test]
fn lifecycle_success_messages_match_their_commands() {
    let root = temp_root("lifecycle");
    std::fs::create_dir_all(&root).unwrap();

    assert_message(
        run(
            &root,
            &[
                "add",
                "gs",
                "--allow-missing-loader",
                "--exec",
                "tool",
                "--shell",
                "bash",
            ],
        ),
        "别名已添加。",
    );
    assert_message(run(&root, &["enable", "gs"]), "别名已启用。");
    assert_message(run(&root, &["disable", "gs"]), "别名已禁用。");
    assert_message(
        run(
            &root,
            &[
                "update",
                "gs",
                "--allow-missing-loader",
                "--description",
                "changed",
            ],
        ),
        "别名已更新。",
    );
    assert_message(run(&root, &["rename", "gs", "gs2"]), "别名已改名。");

    // Audit the remaining success messages rather than relying on a code read.
    assert_message(run(&root, &["sync"]), "synced 1 shell results");
    assert_message(
        run(&root, &["reload"]),
        "当前已打开的 Shell 不会自动更新，请执行 reload 命令。",
    );
    let reload = run(&root, &["reload", "--print", "--shell", "bash"]);
    assert!(reload.status.success());
    assert!(
        stdout(&reload).starts_with("source "),
        "{}",
        stdout(&reload)
    );

    let export = root.join("aliases.json");
    let export_arg = export.to_str().unwrap();
    assert_message(
        run(&root, &["export", export_arg]),
        &format!("exported to {export_arg}"),
    );

    assert_message(run(&root, &["remove", "gs2", "--yes"]), "别名已删除。");

    let _ = std::fs::remove_dir_all(root);
}

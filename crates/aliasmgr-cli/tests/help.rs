use aliasmgr_cli::cli::Cli;
use clap::{CommandFactory, Parser};

fn rendered(args: &[&str]) -> String {
    let error = Cli::try_parse_from(args).expect_err("help output is a clap error");
    error.to_string()
}

#[test]
fn clap_definition_is_valid() {
    // Catches duplicate flags, invalid value names, and conflicting settings.
    Cli::command().debug_assert();
}

#[test]
fn top_level_help_is_english_and_has_long_about() {
    let command = Cli::command();
    let about = command.get_about().expect("top-level about").to_string();
    assert!(
        about.contains("Cross-platform command alias manager"),
        "{about}"
    );
    assert!(!about.contains("跨平台"));
    assert!(
        command.get_long_about().is_some(),
        "top-level long_about is required"
    );

    let long = rendered(&["aliasmgr", "--help"]);
    for expected in [
        "Alias Manager stores command aliases",
        "EXAMPLES",
        "add",
        "sync",
    ] {
        assert!(long.contains(expected), "missing {expected} in:\n{long}");
    }
}

#[test]
fn every_subcommand_has_about_and_long_about() {
    fn assert_command(command: &clap::Command, path: &str) {
        for subcommand in command.get_subcommands() {
            let name = subcommand.get_name();
            if name == "help" {
                continue;
            }
            let full = format!("{path} {name}");
            assert!(
                subcommand
                    .get_about()
                    .map(|about| !about.to_string().trim().is_empty())
                    .unwrap_or(false),
                "{full} must have a non-empty about"
            );
            assert!(
                subcommand
                    .get_long_about()
                    .map(|about| !about.to_string().trim().is_empty())
                    .unwrap_or(false),
                "{full} must have a non-empty long_about"
            );
            assert_command(subcommand, &full);
        }
    }
    assert_command(&Cli::command(), "aliasmgr");
}

#[test]
fn every_flag_and_argument_has_help_text() {
    fn assert_command(command: &clap::Command, path: &str) {
        for arg in command.get_arguments() {
            let id = arg.get_id().as_str();
            if id == "help" || id == "version" {
                continue;
            }
            assert!(
                arg.get_help().is_some() || arg.get_long_help().is_some(),
                "{path}: argument `{id}` is missing help text"
            );
        }
        for subcommand in command.get_subcommands() {
            assert_command(subcommand, &format!("{path} {}", subcommand.get_name()));
        }
    }
    assert_command(&Cli::command(), "aliasmgr");
}

#[test]
fn add_short_and_long_help_show_usage_flags_and_examples() {
    for args in [
        vec!["aliasmgr", "add", "-h"],
        vec!["aliasmgr", "add", "--help"],
        vec!["aliasmgr", "help", "add"],
    ] {
        let output = rendered(&args);
        assert!(output.contains("Create a new alias"), "{output}");
        assert!(output.contains("--exec"), "{output}");
        assert!(output.contains("--target-type"), "{output}");
        assert!(output.contains("--restrict-path"), "{output}");
        assert!(output.contains("EXAMPLES"), "{output}");
        assert!(output.contains("aliasmgr add gs --exec git"), "{output}");
    }
}

#[test]
fn update_import_and_export_help_contain_examples_and_key_flags() {
    let update = rendered(&["aliasmgr", "update", "--help"]);
    for expected in ["--exec", "--clear-restriction", "--pass-args", "EXAMPLES"] {
        assert!(
            update.contains(expected),
            "missing {expected} in:\n{update}"
        );
    }
    let import = rendered(&["aliasmgr", "import", "--help"]);
    for expected in ["--yes", "EXAMPLES", "aliases.toml"] {
        assert!(
            import.contains(expected),
            "missing {expected} in:\n{import}"
        );
    }
    let export = rendered(&["aliasmgr", "export", "--help"]);
    for expected in ["FILE", "EXAMPLES", "aliases.toml"] {
        assert!(
            export.contains(expected),
            "missing {expected} in:\n{export}"
        );
    }
}

#[test]
fn reload_help_explains_dot_sourcing_not_a_comma() {
    let output = rendered(&["aliasmgr", "reload", "--help"]);
    assert!(output.contains("dot-source operator"), "{output}");
    assert!(output.contains("not punctuation"), "{output}");
    assert!(output.contains("--print"), "{output}");
    assert!(output.contains("--shell"), "{output}");
}

#[test]
fn every_subcommand_help_renders_without_panicking() {
    for subcommand in Cli::command()
        .get_subcommands()
        .map(|s| s.get_name().to_string())
        .collect::<Vec<_>>()
    {
        if subcommand == "help" {
            continue;
        }
        let output = rendered(&["aliasmgr", &subcommand, "--help"]);
        assert!(
            output.contains(&subcommand) || output.contains("Usage"),
            "{subcommand} help looks empty:\n{output}"
        );
    }
}

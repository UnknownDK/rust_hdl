use assert_cmd::{cargo::cargo_bin_cmd, Command};
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;
use vhdl_lang::{FormatConfig, KeywordCase};

const INPUT: &str = "package P is signal a: bit; signal longer: bit; end;";
const SETTINGS: &str = "standard = '2008'\n[format]\nindent_width = 2\nkeyword_case = 'upper'\nalign_declarations = true\nalign_associations = true\nalign_assignments = true\n";
const EXPECTED: &str = "PACKAGE P IS\n  SIGNAL a      : bit;\n  SIGNAL longer : bit;\nEND;\n";

fn formatter() -> Command {
    cargo_bin_cmd!("vhdl_lang")
}

#[test]
fn project_settings_parse_with_defaults_and_strict_validation() {
    let config = FormatConfig::from_toml(SETTINGS).unwrap();
    assert_eq!(config.indent_width, 2);
    assert_eq!(config.keyword_case, KeywordCase::Upper);
    assert_eq!(config.max_width, 100);
    assert_eq!(config.inline_argument_limit, 2);
    assert!(config.align_declarations && config.align_associations && config.align_assignments);
    assert_eq!(
        FormatConfig::from_toml("[libraries]").unwrap(),
        FormatConfig::default()
    );
    for bad in [
        "format = true",
        "[format]\nmax_width = 0",
        "[format]\nmax_width = -1",
        "[format]\nmax_width = 10001",
        "[format]\nmax_width = 20.5",
        "[format]\nindent_width = 33",
        "[format]\nkeyword_case = 'preserve'",
        "[format]\nalign_defaults = true",
        "[format]\nalign_declarations = 'true'",
        "[format]\nalign_assignments = 'true'",
        "[format]\ninline_argument_limit = -1",
        "[format]\ninline_argument_limit = 10001",
        "[format]\ninline_argument_limit = '2'",
    ] {
        assert!(FormatConfig::from_toml(bad).is_err(), "accepted {bad}");
    }
}

#[test]
fn assignment_alignment_is_loaded_and_can_be_overridden() {
    let project = TempDir::new().unwrap();
    fs::write(
        project.path().join("vhdl_ls.toml"),
        "[format]\nalign_assignments = true",
    )
    .unwrap();
    let input =
        "entity e is end; architecture rtl of e is begin a <= '0'; longer_name <= '1'; end;";
    formatter()
        .current_dir(project.path())
        .arg("--format-stdin")
        .write_stdin(input)
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "a           <= '0';\n    longer_name <= '1';",
        ));
    formatter()
        .current_dir(project.path())
        .args(["--format-stdin", "--align-assignments=false"])
        .write_stdin(input)
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "a <= '0';\n    longer_name <= '1';",
        ));
}

#[test]
fn argument_limit_is_loaded_and_overridden_for_both_input_modes() {
    let project = TempDir::new().unwrap();
    fs::write(
        project.path().join("vhdl_ls.toml"),
        "[format]\ninline_argument_limit = 0",
    )
    .unwrap();
    let input = "entity e is end; architecture rtl of e is begin foo(a, b); end;";
    let source = project.path().join("example.vhd");
    fs::write(&source, input).unwrap();
    for stdin in [true, false] {
        for limit in [None, Some("2")] {
            let mut cmd = formatter();
            cmd.current_dir(project.path());
            if stdin {
                cmd.arg("--format-stdin").write_stdin(input);
            } else {
                cmd.arg("--format").arg(&source);
            }
            if let Some(limit) = limit {
                cmd.args(["--inline-argument-limit", limit]);
            }
            cmd.assert()
                .success()
                .stderr("")
                .stdout(predicate::str::contains(if limit.is_some() {
                    "foo(a, b);"
                } else {
                    "foo(\n        a,\n        b\n    );"
                }));
        }
    }
    formatter()
        .args(["--format-stdin", "--inline-argument-limit", "10001"])
        .write_stdin(input)
        .assert()
        .code(2)
        .stdout("")
        .stderr(predicate::str::contains("inline_argument_limit"));
}

#[test]
fn aggregates_use_existing_project_settings_and_cli_overrides() {
    let project = TempDir::new().unwrap();
    fs::write(
        project.path().join("vhdl_ls.toml"),
        "[format]\ninline_argument_limit = 2\nalign_associations = true",
    )
    .unwrap();
    let input =
        "entity e is end; architecture rtl of e is begin x <= (a => 1, longer => 2, c => 3); end;";
    let source = project.path().join("example.vhd");
    fs::write(&source, input).unwrap();
    for stdin in [true, false] {
        for flags in [
            vec![],
            vec!["--align-associations=false"],
            vec!["--inline-argument-limit", "3"],
        ] {
            let mut cmd = formatter();
            cmd.current_dir(project.path()).args(&flags);
            if stdin {
                cmd.arg("--format-stdin").write_stdin(input);
            } else {
                cmd.arg("--format").arg(&source);
            }
            let expected = if flags.contains(&"3") {
                "x <= (a => 1, longer => 2, c => 3);"
            } else if flags.is_empty() {
                "x <= (\n        a      => 1,\n        longer => 2,\n        c      => 3\n    );"
            } else {
                "x <= (\n        a => 1,\n        longer => 2,\n        c => 3\n    );"
            };
            cmd.assert()
                .success()
                .stderr("")
                .stdout(predicate::str::contains(expected));
        }
    }
    assert_eq!(fs::read_to_string(&source).unwrap(), input);
}

#[test]
fn stdin_and_files_discover_project_settings_from_the_source_path() {
    let project = TempDir::new().unwrap();
    let elsewhere = TempDir::new().unwrap();
    fs::create_dir(project.path().join("rtl")).unwrap();
    fs::write(project.path().join("vhdl_ls.toml"), SETTINGS).unwrap();
    // The unsaved document need not exist; its path must not be read as input.
    formatter()
        .current_dir(elsewhere.path())
        .args(["--format-stdin", "--stdin-filepath"])
        .arg(project.path().join("rtl/new.vhd"))
        .write_stdin(INPUT)
        .assert()
        .success()
        .stdout(EXPECTED)
        .stderr("");
    let source = project.path().join("rtl/saved.vhd");
    fs::write(&source, INPUT).unwrap();
    formatter()
        .current_dir(elsewhere.path())
        .arg("--format")
        .arg(&source)
        .assert()
        .success()
        .stdout(EXPECTED)
        .stderr("");
    assert_eq!(fs::read_to_string(source).unwrap(), INPUT);
}

#[test]
fn nearest_config_wins_without_merging_and_cli_can_override() {
    let project = TempDir::new().unwrap();
    fs::create_dir(project.path().join("rtl")).unwrap();
    fs::write(project.path().join("vhdl_ls.toml"), SETTINGS).unwrap();
    fs::write(
        project.path().join("rtl/vhdl_ls.toml"),
        "[format]\nindent_width = 0\n",
    )
    .unwrap();
    formatter()
        .current_dir(project.path())
        .args(["--format-stdin", "--stdin-filepath", "rtl/new.vhd"])
        .write_stdin(INPUT)
        .assert()
        .success()
        .stdout("package P is\nsignal a: bit;\nsignal longer: bit;\nend;\n");
    formatter()
        .current_dir(project.path())
        .args([
            "--format-stdin",
            "--keyword-case",
            "lower",
            "--indent-width",
            "4",
            "--align-declarations=false",
        ])
        .write_stdin(INPUT)
        .assert()
        .success()
        .stdout("package P is\n    signal a: bit;\n    signal longer: bit;\nend;\n");
}

#[test]
fn explicit_config_and_opt_out_are_supported() {
    let project = TempDir::new().unwrap();
    fs::write(project.path().join("custom.toml"), SETTINGS).unwrap();
    fs::write(project.path().join("vhdl_ls.toml"), "broken TOML [").unwrap();
    formatter()
        .current_dir(project.path())
        .args(["--format-stdin", "--format-config", "custom.toml"])
        .write_stdin(INPUT)
        .assert()
        .success()
        .stdout(EXPECTED);
    formatter()
        .current_dir(project.path())
        .args([
            "--format-stdin",
            "--no-format-config",
            "--align-declarations",
            "--keyword-case",
            "upper",
            "--indent-width",
            "2",
        ])
        .write_stdin(INPUT)
        .assert()
        .success()
        .stdout(EXPECTED);
    formatter()
        .current_dir(project.path())
        .args(["--format-stdin", "--format-config", "missing.toml"])
        .write_stdin(INPUT)
        .assert()
        .code(2)
        .stdout("")
        .stderr(predicate::str::contains("missing.toml"));
    formatter()
        .current_dir(project.path())
        .arg("--format-stdin")
        .write_stdin(INPUT)
        .assert()
        .code(2)
        .stdout("")
        .stderr(predicate::str::contains("vhdl_ls.toml"));
}

#[test]
fn command_line_values_are_validated_before_formatting() {
    for args in [["--max-width", "0"], ["--indent-width", "999999999"]] {
        formatter()
            .args(["--format-stdin", "--no-format-config"])
            .args(args)
            .write_stdin(INPUT)
            .assert()
            .code(2)
            .stdout("")
            .stderr(predicate::str::contains("formatter configuration"));
    }
}

#[test]
fn project_standard_is_used_and_can_be_overridden() {
    let project = TempDir::new().unwrap();
    fs::write(project.path().join("vhdl_ls.toml"), "standard = '2019'").unwrap();
    let input = "package p is type pair is record a: bit; b: bit; end record; view pair_view of pair is a: in; b: out; end view; end package;";
    formatter()
        .current_dir(project.path())
        .arg("--format-stdin")
        .write_stdin(input)
        .assert()
        .success()
        .stderr("");
    formatter()
        .current_dir(project.path())
        .args(["--format-stdin", "--standard", "2008"])
        .write_stdin(input)
        .assert()
        .failure()
        .stdout("");
}

#[test]
fn relative_parent_paths_do_not_discover_the_wrong_project() {
    let root = TempDir::new().unwrap();
    fs::create_dir(root.path().join("first")).unwrap();
    fs::create_dir(root.path().join("second")).unwrap();
    fs::write(root.path().join("first/vhdl_ls.toml"), SETTINGS).unwrap();
    formatter()
        .current_dir(root.path().join("first"))
        .args(["--format-stdin", "--stdin-filepath", "../second/new.vhd"])
        .write_stdin(INPUT)
        .assert()
        .success()
        .stdout(predicate::str::starts_with("package P is"));
}

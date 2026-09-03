use assert_cmd::{cargo::cargo_bin_cmd, Command};
use predicates::prelude::*;
use std::fs;
use tempfile::NamedTempFile;

const INPUT: &str = "entity foo is\nport(\na:in std_logic\n);\nend entity;";
const OTHER_INPUT: &str = "entity bar is\nend entity;";
const EXPECTED: &str = "entity foo is\n    port (\n        a: in std_logic\n    );\nend entity;\n";

fn formatter() -> Command {
    cargo_bin_cmd!("vhdl_lang")
}

#[test]
fn formats_stdin_to_exact_stdout() {
    formatter()
        .arg("--format-stdin")
        .write_stdin(INPUT)
        .assert()
        .success()
        .stdout(EXPECTED)
        .stderr("");
}

#[test]
fn sends_parse_errors_to_stderr_only() {
    formatter()
        .arg("--format-stdin")
        .write_stdin("entity")
        .assert()
        .failure()
        .stdout("")
        .stderr(predicate::str::contains("parse diagnostic"))
        .stderr(predicate::str::contains("error:"));
}

#[test]
fn accepts_empty_stdin() {
    formatter()
        .arg("--format-stdin")
        .write_stdin("")
        .assert()
        .success()
        .stdout("")
        .stderr("");
}

#[test]
fn stdin_filepath_is_metadata_only() {
    let file = NamedTempFile::new().unwrap();
    fs::write(file.path(), OTHER_INPUT).unwrap();

    formatter()
        .args(["--format-stdin", "--stdin-filepath"])
        .arg(file.path())
        .write_stdin(INPUT)
        .assert()
        .success()
        .stdout(EXPECTED)
        .stderr("");

    assert_eq!(fs::read_to_string(file.path()).unwrap(), OTHER_INPUT);
}

#[test]
fn file_and_stdin_modes_have_identical_output() {
    let file = NamedTempFile::new().unwrap();
    fs::write(file.path(), INPUT).unwrap();

    let file_output = formatter()
        .arg("--format")
        .arg(file.path())
        .output()
        .unwrap();
    assert!(file_output.status.success());
    assert!(file_output.stderr.is_empty());

    formatter()
        .args(["--format-stdin", "--stdin-filepath"])
        .arg(file.path())
        .write_stdin(INPUT)
        .assert()
        .success()
        .stdout(file_output.stdout)
        .stderr("");
}

#[test]
fn stdin_filepath_requires_stdin_mode() {
    formatter()
        .arg("--stdin-filepath")
        .arg("unused.vhd")
        .assert()
        .failure()
        .stderr(predicate::str::contains("--format-stdin"));
}

use assert_cmd::{cargo::cargo_bin_cmd, Command};
use predicates::prelude::*;
use std::fs;
use tempfile::NamedTempFile;

const INPUT: &str = "entity foo is\nport(\na:in std_logic\n);\nend entity;";
const OTHER_INPUT: &str = "entity bar is\nend entity;";
const EXPECTED: &str = "entity foo is\n    port (a: in std_logic);\nend entity;\n";

fn formatter() -> Command {
    let mut command = cargo_bin_cmd!("vhdl_lang");
    // Discovery has its own tests; these checks must not inherit local settings.
    command.arg("--no-format-config");
    command
}

#[test]
fn unicode_file_and_stdin_input_preserve_literals_and_comments() {
    let input = "package p is\nconstant greeting: string := \"café\"; -- æ € 😀\nend;\n";
    let file = NamedTempFile::new().unwrap();
    fs::write(file.path(), input).unwrap();
    let output = formatter()
        .arg("--format")
        .arg(file.path())
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout.clone()).unwrap();
    assert!(text.contains("\"café\"; -- æ € 😀"), "{text}");
    formatter()
        .arg("--format-stdin")
        .write_stdin(input)
        .assert()
        .success()
        .stdout(output.stdout)
        .stderr("");
    assert_eq!(fs::read_to_string(file.path()).unwrap(), input);
}

#[test]
fn legacy_latin1_requires_an_explicit_encoding_in_both_modes() {
    let input = b"package p is constant greeting: string := \"caf\xe9\"; end;";
    let file = NamedTempFile::new().unwrap();
    fs::write(file.path(), input).unwrap();
    for stdin in [false, true] {
        for latin1 in [false, true] {
            let mut cmd = formatter();
            if latin1 {
                cmd.args(["--input-encoding", "latin1"]);
            }
            if stdin {
                cmd.arg("--format-stdin").write_stdin(input.as_slice());
            } else {
                cmd.arg("--format").arg(file.path());
            }
            if latin1 {
                cmd.assert()
                    .success()
                    .stderr("")
                    .stdout(predicate::str::contains("\"café\""));
            } else {
                cmd.assert()
                    .code(2)
                    .stdout("")
                    .stderr(predicate::str::contains("--input-encoding latin1"));
            }
        }
    }
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

#[test]
fn exposes_keyword_case_width_and_indentation() {
    formatter()
        .args([
            "--format-stdin",
            "--keyword-case",
            "upper",
            "--indent-width",
            "2",
            "--max-width",
            "25",
        ])
        .write_stdin("entity Foo is port(a: in bit); end;")
        .assert()
        .success()
        .stdout("ENTITY Foo IS\n  PORT (a: IN bit);\nEND;\n")
        .stderr("");
}

#[test]
fn rejects_unknown_keyword_case() {
    formatter()
        .args(["--format-stdin", "--keyword-case", "preserve"])
        .write_stdin(INPUT)
        .assert()
        .failure()
        .stdout("")
        .stderr(predicate::str::contains("lower, upper"));
}

#[test]
fn configured_file_and_stdin_modes_match() {
    let file = NamedTempFile::new().unwrap();
    fs::write(file.path(), INPUT).unwrap();
    let output = formatter()
        .args(["--keyword-case", "upper", "--max-width", "20", "--format"])
        .arg(file.path())
        .output()
        .unwrap();
    assert!(output.status.success());
    formatter()
        .args([
            "--keyword-case",
            "upper",
            "--max-width",
            "20",
            "--format-stdin",
        ])
        .write_stdin(INPUT)
        .assert()
        .success()
        .stdout(output.stdout)
        .stderr("");
}

#[test]
fn disabled_region_crlf_text_survives_stdin_and_file_formatting() {
    let input = "ENTITY Foo IS\r\n  --vhdl_ls off\r\n  invalid \" \r\n--vhdl_ls on\r\nEND;\r\n";
    let expected = "entity Foo is\n  --vhdl_ls off\r\n  invalid \" \r\n--vhdl_ls on\r\nend;\n";
    formatter()
        .arg("--format-stdin")
        .write_stdin(input)
        .assert()
        .success()
        .stdout(expected);
    formatter()
        .arg("--format-stdin")
        .write_stdin(expected)
        .assert()
        .success()
        .stdout(expected);
    let file = NamedTempFile::new().unwrap();
    fs::write(file.path(), input).unwrap();
    formatter()
        .arg("--format")
        .arg(file.path())
        .assert()
        .success()
        .stdout(expected);
    assert_eq!(fs::read_to_string(file.path()).unwrap(), input);
}

#[test]
fn block_disabled_regions_preserve_raw_text_and_resume_formatting() {
    for ending in ["\n", "\r\n"] {
        for closing in [
            "/*\nvhdl_ls on\n*/",
            "/* ordinary */ /* vhdl_ls on */",
            "/* ordinary\n-- vhdl_ls on\n*/ /* vhdl_ls on */",
            "-- prose /* vhdl_ls on */\n/* vhdl_ls on */",
            "/* vhdl_ls on extra */\n/* vhdl_ls on */",
        ] {
            let raw = format!("/* vhdl_ls off */\ninvalid € \"\n{closing}\n").replace('\n', ending);
            let input = format!("ENTITY e IS{ending}{raw}END ENTITY;{ending}");
            let expected = format!("entity e is\n{raw}end entity;\n");
            let file = NamedTempFile::new().unwrap();
            fs::write(file.path(), &input).unwrap();
            formatter()
                .arg("--format")
                .arg(file.path())
                .assert()
                .success()
                .stdout(expected.clone());
            for text in [&input, &expected] {
                formatter()
                    .arg("--format-stdin")
                    .write_stdin(text.as_bytes())
                    .assert()
                    .success()
                    .stdout(expected.clone());
            }
        }
    }
}

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this file,
// You can obtain one at http://mozilla.org/MPL/2.0/.

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

const UNFORMATTED: &str = "entity   a is end entity;\n";
const FORMATTED: &str = "entity a is\nend entity;\n";
const BROKEN: &str = "entity b is end\n";

fn project() -> TempDir {
    let directory = TempDir::new().unwrap();
    fs::create_dir(directory.path().join("sub")).unwrap();
    fs::write(directory.path().join("a.vhd"), UNFORMATTED).unwrap();
    fs::write(directory.path().join("sub/c.VHDL"), UNFORMATTED).unwrap();
    fs::write(directory.path().join("sub/done.vhd"), FORMATTED).unwrap();
    fs::write(directory.path().join("notes.txt"), UNFORMATTED).unwrap();
    directory
}

fn vhdl_fmt(directory: &TempDir) -> Command {
    let mut command = Command::cargo_bin("vhdl-fmt").unwrap();
    command.current_dir(directory.path());
    command
}

#[test]
fn formats_a_directory_in_place() {
    let directory = project();
    vhdl_fmt(&directory)
        .assert()
        .success()
        .stdout("2 files reformatted, 1 file left unchanged\n");
    assert_eq!(
        fs::read_to_string(directory.path().join("a.vhd")).unwrap(),
        FORMATTED
    );
    assert_eq!(
        fs::read_to_string(directory.path().join("sub/c.VHDL")).unwrap(),
        FORMATTED
    );
    assert_eq!(
        fs::read_to_string(directory.path().join("notes.txt")).unwrap(),
        UNFORMATTED
    );
    vhdl_fmt(&directory)
        .assert()
        .success()
        .stdout("0 files reformatted, 3 files left unchanged\n");
}

#[test]
fn check_lists_changes_without_writing() {
    let directory = project();
    vhdl_fmt(&directory).arg("--check").assert().code(1).stdout(
        "Would reformat: ./a.vhd\nWould reformat: ./sub/c.VHDL\n\
             2 files would be reformatted, 1 file already formatted\n",
    );
    assert_eq!(
        fs::read_to_string(directory.path().join("a.vhd")).unwrap(),
        UNFORMATTED
    );
}

#[test]
fn diff_prints_a_patch_without_writing() {
    let directory = project();
    vhdl_fmt(&directory)
        .args(["--diff", "a.vhd"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains(
            "-entity   a is end entity;\n+entity a is\n+end entity;\n",
        ));
    assert_eq!(
        fs::read_to_string(directory.path().join("a.vhd")).unwrap(),
        UNFORMATTED
    );
}

#[test]
fn a_file_that_fails_does_not_stop_the_others() {
    let directory = project();
    fs::write(directory.path().join("b.vhd"), BROKEN).unwrap();
    vhdl_fmt(&directory)
        .assert()
        .code(2)
        .stdout("2 files reformatted, 1 file left unchanged, 1 file failed\n")
        .stderr(predicate::str::contains(
            "error: ./b.vhd:1:16: expected ';'",
        ));
    assert_eq!(
        fs::read_to_string(directory.path().join("a.vhd")).unwrap(),
        FORMATTED
    );
    assert_eq!(
        fs::read_to_string(directory.path().join("b.vhd")).unwrap(),
        BROKEN
    );
}

#[test]
fn excluded_paths_are_skipped() {
    let directory = project();
    vhdl_fmt(&directory)
        .args(["--exclude", "sub/"])
        .assert()
        .success()
        .stdout("1 file reformatted, 0 files left unchanged\n");
}

#[test]
fn standard_input_is_formatted_to_standard_output() {
    let directory = project();
    vhdl_fmt(&directory)
        .arg("-")
        .write_stdin(UNFORMATTED)
        .assert()
        .success()
        .stdout(FORMATTED);
    vhdl_fmt(&directory)
        .args(["-", "--check"])
        .write_stdin(UNFORMATTED)
        .assert()
        .code(1);
    vhdl_fmt(&directory)
        .args(["-", "--stdin-filename", "x.vhd"])
        .write_stdin(BROKEN)
        .assert()
        .code(2)
        .stdout("")
        .stderr("error: x.vhd:1:16: expected ';'\n");
}

#[test]
fn the_nearest_project_file_configures_each_file() {
    let directory = project();
    fs::write(
        directory.path().join("sub/vhdl_ls.toml"),
        "[format]\nkeyword_case = \"upper\"\n",
    )
    .unwrap();
    vhdl_fmt(&directory).assert().success();
    assert_eq!(
        fs::read_to_string(directory.path().join("a.vhd")).unwrap(),
        FORMATTED
    );
    assert_eq!(
        fs::read_to_string(directory.path().join("sub/c.VHDL")).unwrap(),
        "ENTITY a IS\nEND ENTITY;\n"
    );
    // Command-line options override the project file; --isolated ignores it.
    vhdl_fmt(&directory)
        .args(["--isolated", "sub/c.VHDL"])
        .assert()
        .success();
    assert_eq!(
        fs::read_to_string(directory.path().join("sub/c.VHDL")).unwrap(),
        FORMATTED
    );
}

#[test]
fn an_invalid_project_file_fails_only_its_files() {
    let directory = project();
    fs::write(
        directory.path().join("sub/vhdl_ls.toml"),
        "[format]\nwidth = 3\n",
    )
    .unwrap();
    vhdl_fmt(&directory)
        .assert()
        .code(2)
        .stdout("1 file reformatted, 0 files left unchanged, 2 files failed\n")
        .stderr(predicate::str::contains("unknown field `width`"));
}

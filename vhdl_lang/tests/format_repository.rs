use assert_cmd::{cargo::cargo_bin_cmd, Command};
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

const INPUT: &str = "ENTITY e IS END;";
const OUTPUT: &str = "entity e is\nend;\n";

fn command(project: &TempDir) -> Command {
    let mut command = cargo_bin_cmd!("vhdl_lang");
    command
        .current_dir(project.path())
        .arg("--no-format-config");
    command
}

#[test]
fn recursive_check_diff_and_write_form_a_complete_workflow() {
    let project = TempDir::new().unwrap();
    fs::create_dir_all(project.path().join("rtl/nested")).unwrap();
    fs::write(project.path().join("rtl/a.vhd"), INPUT).unwrap();
    fs::write(project.path().join("rtl/nested/b.VHDL"), INPUT).unwrap();
    fs::write(project.path().join("rtl/notes.txt"), "not VHDL").unwrap();
    command(&project)
        .args(["--format", "rtl", "--check"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("Would reformat rtl/a.vhd"));
    command(&project)
        .args(["--format", "rtl", "--diff"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains(
            "--- a/rtl/a.vhd\n+++ b/rtl/a.vhd\n",
        ))
        .stdout(predicate::str::contains("-ENTITY e IS END;"));
    assert_eq!(
        fs::read_to_string(project.path().join("rtl/a.vhd")).unwrap(),
        INPUT
    );
    command(&project)
        .args(["--format", "rtl", "--write"])
        .assert()
        .success()
        .stdout("");
    for path in ["rtl/a.vhd", "rtl/nested/b.VHDL"] {
        assert_eq!(
            fs::read_to_string(project.path().join(path)).unwrap(),
            OUTPUT
        );
    }
    assert_eq!(
        fs::read_to_string(project.path().join("rtl/notes.txt")).unwrap(),
        "not VHDL"
    );
    command(&project)
        .args(["--format", "rtl", "--check"])
        .assert()
        .success()
        .stdout("");
    command(&project)
        .args(["--format", "rtl", "--diff"])
        .assert()
        .success()
        .stdout("");
}

#[test]
fn multiple_paths_are_deduplicated_and_exclusions_apply_to_explicit_files() {
    let project = TempDir::new().unwrap();
    fs::create_dir_all(project.path().join("rtl/vendor")).unwrap();
    fs::create_dir(project.path().join("rtl/.git")).unwrap();
    for path in [
        "rtl/a.vhd",
        "rtl/b.vhd",
        "rtl/vendor/bad.vhd",
        "rtl/.git/bad.vhd",
    ] {
        fs::write(
            project.path().join(path),
            if path.contains("bad") {
                "invalid"
            } else {
                INPUT
            },
        )
        .unwrap();
    }
    let result = command(&project)
        .args([
            "--format",
            "rtl",
            "rtl/a.vhd",
            "rtl/b.vhd",
            "--check",
            "--exclude",
            "vendor/**",
            "--exclude",
            "b.vhd",
        ])
        .assert()
        .code(1)
        .get_output()
        .stdout
        .clone();
    assert_eq!(
        String::from_utf8(result).unwrap(),
        "Would reformat rtl/a.vhd\n"
    );
}

#[test]
fn failures_do_not_write_or_emit_partial_diffs() {
    let project = TempDir::new().unwrap();
    fs::write(project.path().join("a.vhd"), INPUT).unwrap();
    fs::write(project.path().join("z.vhd"), "entity broken").unwrap();
    for mode in ["--write", "--check", "--diff"] {
        command(&project)
            .args(["--format", ".", mode])
            .assert()
            .code(2)
            .stdout("");
        assert_eq!(
            fs::read_to_string(project.path().join("a.vhd")).unwrap(),
            INPUT
        );
        assert_eq!(
            fs::read_to_string(project.path().join("z.vhd")).unwrap(),
            "entity broken"
        );
        assert_eq!(fs::read_dir(project.path()).unwrap().count(), 2);
    }
}

#[test]
fn batch_uses_each_files_own_project_settings() {
    let project = TempDir::new().unwrap();
    for (dir, case) in [("one", "lower"), ("two", "upper")] {
        fs::create_dir(project.path().join(dir)).unwrap();
        fs::write(
            project.path().join(dir).join("vhdl_ls.toml"),
            format!("[format]\nkeyword_case = '{case}'"),
        )
        .unwrap();
        fs::write(project.path().join(dir).join("e.vhd"), INPUT).unwrap();
    }
    cargo_bin_cmd!("vhdl_lang")
        .current_dir(project.path())
        .args(["--format", "one", "two", "--write"])
        .assert()
        .success();
    assert_eq!(
        fs::read_to_string(project.path().join("one/e.vhd")).unwrap(),
        OUTPUT
    );
    assert_eq!(
        fs::read_to_string(project.path().join("two/e.vhd")).unwrap(),
        "ENTITY e IS\nEND;\n"
    );
}

#[test]
fn stdin_supports_check_and_diff_and_rejects_writing() {
    let project = TempDir::new().unwrap();
    for mode in ["--check", "--diff"] {
        command(&project)
            .args(["--format-stdin", "--stdin-filepath", "buffer.vhd", mode])
            .write_stdin(INPUT)
            .assert()
            .code(1)
            .stdout(predicate::str::contains("buffer.vhd"));
        command(&project)
            .args(["--format-stdin", mode])
            .write_stdin(OUTPUT)
            .assert()
            .success()
            .stdout("");
    }
    command(&project)
        .args(["--format-stdin", "--write"])
        .assert()
        .code(2)
        .stdout("");
}

#[test]
fn single_file_stdout_remains_compatible_and_batch_requires_an_explicit_mode() {
    let project = TempDir::new().unwrap();
    fs::write(project.path().join("e.vhd"), INPUT).unwrap();
    command(&project)
        .args(["--format", "e.vhd"])
        .assert()
        .success()
        .stdout(OUTPUT);
    command(&project)
        .args(["--format", "."])
        .assert()
        .code(2)
        .stdout("");
    command(&project)
        .args(["--format", "e.vhd", "e.vhd"])
        .assert()
        .code(2)
        .stdout("");
    assert_eq!(
        fs::read_to_string(project.path().join("e.vhd")).unwrap(),
        INPUT
    );
}

#[test]
fn latin1_in_place_writes_preserve_encoding() {
    let project = TempDir::new().unwrap();
    fs::write(
        project.path().join("e.vhd"),
        b"ENTITY e IS -- caf\xe9\nEND;",
    )
    .unwrap();
    command(&project)
        .args(["--format", "e.vhd", "--write", "--input-encoding", "latin1"])
        .assert()
        .success();
    assert_eq!(
        fs::read(project.path().join("e.vhd")).unwrap(),
        b"entity e is -- caf\xe9\nend;\n"
    );
    command(&project)
        .args(["--format", "e.vhd", "--check", "--input-encoding", "latin1"])
        .assert()
        .success();
}

#[test]
fn empty_directories_invalid_globs_missing_paths_and_conflicting_modes() {
    let project = TempDir::new().unwrap();
    command(&project)
        .args(["--format", ".", "--check"])
        .assert()
        .success()
        .stdout("");
    command(&project)
        .args(["--format", ".", "--check", "--exclude", "["])
        .assert()
        .code(2);
    command(&project)
        .args(["--format", "missing", "--write"])
        .assert()
        .code(2);
    for pair in [
        ["--write", "--check"],
        ["--write", "--diff"],
        ["--check", "--diff"],
    ] {
        command(&project)
            .args(["--format", "."])
            .args(pair)
            .assert()
            .code(2);
    }
}

#[cfg(unix)]
#[test]
fn writes_preserve_permissions_and_reject_links_and_readonly_files() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let project = TempDir::new().unwrap();
    let path = project.path().join("e.vhd");
    fs::write(&path, INPUT).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
    command(&project)
        .args(["--format", "e.vhd", "--write"])
        .assert()
        .success();
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o640
    );
    fs::write(&path, INPUT).unwrap();
    symlink("e.vhd", project.path().join("link.vhd")).unwrap();
    command(&project)
        .args(["--format", "link.vhd", "--write"])
        .assert()
        .code(2);
    assert!(fs::symlink_metadata(project.path().join("link.vhd"))
        .unwrap()
        .file_type()
        .is_symlink());
    fs::hard_link(&path, project.path().join("hard.vhd")).unwrap();
    command(&project)
        .args(["--format", "e.vhd", "--write"])
        .assert()
        .code(2);
    assert_eq!(fs::read_to_string(&path).unwrap(), INPUT);
    let readonly = project.path().join("readonly.vhd");
    fs::write(&readonly, INPUT).unwrap();
    fs::set_permissions(&readonly, fs::Permissions::from_mode(0o444)).unwrap();
    command(&project)
        .args(["--format", "readonly.vhd", "--write"])
        .assert()
        .code(2);
    assert_eq!(fs::read_to_string(&readonly).unwrap(), INPUT);
}

#[cfg(unix)]
#[test]
fn large_batches_do_not_require_a_file_descriptor_per_input() {
    let project = TempDir::new().unwrap();
    for index in 0..96 {
        fs::write(project.path().join(format!("{index}.vhd")), INPUT).unwrap();
    }
    Command::new("sh")
        .current_dir(project.path())
        .args([
            "-c",
            "ulimit -n 64; exec \"$1\" --no-format-config --format . --write",
            "formatter-test",
        ])
        .arg(assert_cmd::cargo::cargo_bin!("vhdl_lang"))
        .assert()
        .success();
    command(&project)
        .args(["--format", ".", "--check"])
        .assert()
        .success();
    assert_eq!(fs::read_dir(project.path()).unwrap().count(), 96);
}

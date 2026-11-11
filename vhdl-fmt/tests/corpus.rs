// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this file,
// You can obtain one at http://mozilla.org/MPL/2.0/.

//! Formats real VHDL and checks that every file is either rejected by the
//! parser or formatted safely and idempotently.
//!
//! Set `VHDL_FMT_CORPUS` to a list of directories (separated like `PATH`) to
//! include other source trees, for example a simulator's test suite.

use std::path::{Path, PathBuf};
use vhdl_fmt::{format, FormatError, KeywordCase, Settings};
use vhdl_syntax::standard::VHDLStandard;

fn vhdl_files(root: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            vhdl_files(&path, files);
        } else if path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| matches!(extension, "vhd" | "vhdl"))
        {
            files.push(path);
        }
    }
}

/// Returns whether the file parsed. Panics if it parsed but did not format
/// safely and idempotently.
fn check(path: &Path) -> bool {
    let input = std::fs::read(path).unwrap();
    let mut parsed = false;
    for standard in [VHDLStandard::VHDL2008, VHDLStandard::VHDL2019] {
        for keyword_case in [KeywordCase::Lower, KeywordCase::Upper] {
            let mut settings = Settings {
                standard,
                ..Settings::default()
            };
            settings.format.keyword_case = keyword_case;
            match format(&input, &settings) {
                Ok(output) => {
                    parsed = true;
                    let again = format(&output, &settings).unwrap_or_else(|error| {
                        panic!("{}: formatted output fails: {error}", path.display())
                    });
                    assert!(again == output, "{}: not idempotent", path.display());
                }
                Err(FormatError::Syntax(_)) => {}
                Err(error) => panic!("{}: {error}", path.display()),
            }
        }
    }
    parsed
}

#[test]
fn sources_in_this_repository_format_safely() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let mut files = Vec::new();
    for directory in ["vhdl_libraries", "example_project"] {
        vhdl_files(&repository.join(directory), &mut files);
    }
    assert!(files.len() >= 30, "the corpus is missing");
    for file in &files {
        assert!(check(file), "{} does not parse", file.display());
    }
}

#[test]
fn external_corpus_formats_safely() {
    let Some(roots) = std::env::var_os("VHDL_FMT_CORPUS") else {
        return;
    };
    let mut files = Vec::new();
    for root in std::env::split_paths(&roots) {
        vhdl_files(&root, &mut files);
    }
    let parsed = files.iter().filter(|file| check(file)).count();
    eprintln!("{parsed} of {} files parse and format safely", files.len());
    assert!(parsed > 0, "VHDL_FMT_CORPUS holds no parseable VHDL");
}

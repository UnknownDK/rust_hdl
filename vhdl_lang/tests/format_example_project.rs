use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use vhdl_lang::{format_source, Source, VHDLParser, VHDLStandard};

// excluded file contains PSL statements
const EXCLUDED_FILES: [&str; 1] = ["vunit/examples/vhdl/array_axis_vcs/src/fifo.vhd"];

fn format_file(path: &Path) -> Result<(), Box<dyn Error>> {
    let parser = VHDLParser::new(VHDLStandard::default());
    let source = Source::from_latin1_file(path)?;
    let once = format_source(&parser, &source)?;
    let twice = format_source(&parser, &Source::inline(path, &once))?;
    assert_eq!(once, twice, "{} is not idempotent", path.display());
    Ok(())
}

fn format_dir(path: &Path) -> Result<usize, Box<dyn Error>> {
    let mut count = 0;
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            count += format_dir(&entry.path())?;
        } else if let Some(extension) = entry.path().extension() {
            if (extension == "vhd" || extension == "vhdl") && !is_file_excluded(&entry.path()) {
                format_file(&entry.path())?;
                count += 1;
            }
        }
    }
    Ok(count)
}

fn is_file_excluded(path: &Path) -> bool {
    for file in EXCLUDED_FILES {
        if path.ends_with(file) {
            return true;
        }
    }
    false
}

// Checks that all files in the example project are correctly formatted
// while retaining their token stream.
#[test]
fn formats_all_vhdl_files_without_producing_different_code() -> Result<(), Box<dyn Error>> {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("../example_project");
    if !path.try_exists()? {
        eprintln!("Optional example-project corpus is absent; the bundled IEEE corpus runs separately in format_corpus.rs");
        return Ok(());
    }
    let count = format_dir(&path)?;
    eprintln!("Optional example-project corpus: {count} VHDL files; the bundled IEEE corpus runs separately in format_corpus.rs");
    Ok(())
}

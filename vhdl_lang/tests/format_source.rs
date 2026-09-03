use std::path::Path;
use vhdl_lang::{format_source, FormatError, Source, VHDLParser, VHDLStandard};

const INPUT: &str = "entity foo is\nport(\na:in std_logic\n);\nend entity;";
const EXPECTED: &str = "entity foo is\n    port (\n        a: in std_logic\n    );\nend entity;";

#[test]
fn formats_an_in_memory_source() {
    let source = Source::inline(Path::new("format_source_success.vhd"), INPUT);
    let parser = VHDLParser::new(VHDLStandard::default());

    assert_eq!(format_source(&parser, &source).unwrap(), EXPECTED);
}

#[test]
fn rejects_invalid_vhdl_without_panicking() {
    let source = Source::inline(Path::new("format_source_invalid.vhd"), "entity");
    let parser = VHDLParser::new(VHDLStandard::default());

    assert!(matches!(
        format_source(&parser, &source),
        Err(FormatError::InputDiagnostics(diagnostics)) if !diagnostics.is_empty()
    ));
}

#[test]
fn accepts_an_empty_source() {
    let source = Source::inline(Path::new("format_source_empty.vhd"), "");
    let parser = VHDLParser::new(VHDLStandard::default());

    assert_eq!(format_source(&parser, &source).unwrap(), "");
}

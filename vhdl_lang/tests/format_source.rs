use std::path::Path;
use vhdl_lang::{format_source, FormatError, Source, VHDLParser, VHDLStandard};

const INPUT: &str = "entity foo is\nport(\na:in std_logic\n);\nend entity;";
const EXPECTED: &str = "entity foo is\n    port (\n        a: in std_logic\n    );\nend entity;\n";

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

#[test]
fn preserves_sources_with_formatter_disabled_regions() {
    let input = "architecture rtl of foo is\nbegin\n-- vhdl_ls off\nnot valid vhdl\n-- vhdl_ls on\nend architecture;\n";
    let source = Source::inline(Path::new("format_source_disabled.vhd"), input);
    let parser = VHDLParser::new(VHDLStandard::default());

    assert_eq!(format_source(&parser, &source).unwrap(), input);
}

#[test]
fn preserves_a_comment_only_source() {
    let input = "-- first\n\n\n-- final";
    let source = Source::inline(Path::new("format_source_comments.vhd"), input);
    let parser = VHDLParser::new(VHDLStandard::default());

    assert_eq!(
        format_source(&parser, &source).unwrap(),
        "-- first\n\n-- final\n"
    );
}

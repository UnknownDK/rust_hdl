use std::path::Path;
use vhdl_lang::{
    format_source, format_text_with_config, FormatConfig, FormatError, Source, VHDLParser,
    VHDLStandard,
};

const INPUT: &str = "entity foo is\nport(\na:in std_logic\n);\nend entity;";
const EXPECTED: &str = "entity foo is\n    port (a: in std_logic);\nend entity;\n";

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

#[test]
fn character_literals_do_not_hide_disabled_regions() {
    let path = Path::new("characters_and_disabled_regions.vhd");
    let region = "-- vhdl_ls off\nsignal hidden: bit;\n-- vhdl_ls on\n";
    for standard in [
        VHDLStandard::VHDL1993,
        VHDLStandard::VHDL2008,
        VHDLStandard::VHDL2019,
    ] {
        let parser = VHDLParser::new(standard);
        for character in ['"', '\\', '\'', 'é', '('] {
            for expression in [
                format!("'{character}'"),
                format!("character'('{character}')"),
                format!("character /* qualifier */ '('{character}')"),
                format!("\\character\\'('{character}')"),
            ] {
                let input =
                    format!("package p is\nconstant c: character := {expression};\n{region}end;\n");
                for text_api in [false, true] {
                    let format = |text: &str| {
                        if text_api {
                            format_text_with_config(&parser, path, text, &FormatConfig::default())
                        } else {
                            format_source(&parser, &Source::inline(path, text))
                        }
                        .unwrap()
                    };
                    let output = format(&input);
                    assert!(
                        output.ends_with(&format!("{region}end;\n")),
                        "{standard:?}: {expression}\n{output}"
                    );
                    assert_eq!(format(&output), output);
                }
            }
        }
    }
}

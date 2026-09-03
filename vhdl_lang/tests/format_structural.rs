use std::path::Path;
use vhdl_lang::{format_source, Source, VHDLParser, VHDLStandard};

const FIXTURES: &[(&str, &str)] = &[
    (
        include_str!("formatting/design_units.input.vhd"),
        include_str!("formatting/design_units.expected.vhd"),
    ),
    (
        include_str!("formatting/nesting.input.vhd"),
        include_str!("formatting/nesting.expected.vhd"),
    ),
    (
        include_str!("formatting/declarations.input.vhd"),
        include_str!("formatting/declarations.expected.vhd"),
    ),
    (
        include_str!("formatting/generate.input.vhd"),
        include_str!("formatting/generate.expected.vhd"),
    ),
    (
        include_str!("formatting/comments.input.vhd"),
        include_str!("formatting/comments.expected.vhd"),
    ),
];

#[test]
fn structurally_formats_fixtures_idempotently() {
    let parser = VHDLParser::new(VHDLStandard::default());

    for (index, (input, expected)) in FIXTURES.iter().enumerate() {
        let path = Path::new("formatting_fixture.vhd");
        let once = format_source(&parser, &Source::inline(path, input)).unwrap();
        assert_eq!(&once, expected, "fixture {index}");
        assert!(once.is_empty() || once.ends_with('\n'), "fixture {index}");

        let twice = format_source(&parser, &Source::inline(path, &once)).unwrap();
        assert_eq!(twice, once, "fixture {index} is not idempotent");
    }
}

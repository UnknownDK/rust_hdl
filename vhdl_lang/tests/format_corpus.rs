use std::path::Path;
use vhdl_lang::{
    format_source_with_config, FormatConfig, KeywordCase, Source, VHDLParser, VHDLStandard,
};

// Existing, version-controlled IEEE sources carry Apache-2.0 notices. No network
// or uninitialized submodules are needed, so this corpus cannot silently be empty.
const CORPUS: &[(&str, &str, VHDLStandard)] = &[
    (
        "math_real.vhdl",
        include_str!("formatting/corpus/ieee2008/math_real.vhdl"),
        VHDLStandard::VHDL1993,
    ),
    (
        "math_real-body.vhdl",
        include_str!("formatting/corpus/ieee2008/math_real-body.vhdl"),
        VHDLStandard::VHDL1993,
    ),
    (
        "std_logic_1164.vhdl",
        include_str!("formatting/corpus/ieee2008/std_logic_1164.vhdl"),
        VHDLStandard::VHDL2008,
    ),
    (
        "std_logic_1164-body.vhdl",
        include_str!("formatting/corpus/ieee2008/std_logic_1164-body.vhdl"),
        VHDLStandard::VHDL2008,
    ),
    (
        "numeric_std.vhdl",
        include_str!("formatting/corpus/ieee2008/numeric_std.vhdl"),
        VHDLStandard::VHDL2008,
    ),
    (
        "numeric_std-body.vhdl",
        include_str!("formatting/corpus/ieee2008/numeric_std-body.vhdl"),
        VHDLStandard::VHDL2008,
    ),
];

#[test]
fn real_world_corpus_preserves_tokens_comments_and_is_idempotent() {
    for &(name, input, minimum_standard) in CORPUS {
        for standard in [
            VHDLStandard::VHDL1993,
            VHDLStandard::VHDL2008,
            VHDLStandard::VHDL2019,
        ] {
            if standard < minimum_standard {
                continue;
            }
            for keyword_case in [KeywordCase::Lower, KeywordCase::Upper] {
                for align in [false, true] {
                    let parser = VHDLParser::new(standard);
                    let config = FormatConfig {
                        keyword_case,
                        align_declarations: align,
                        align_associations: align,
                        ..FormatConfig::default()
                    };
                    let output = format_source_with_config(
                        &parser,
                        &Source::inline(Path::new(name), input),
                        &config,
                    )
                    .unwrap_or_else(|error| {
                        panic!("{name} {standard:?} {keyword_case:?}: {error:?}")
                    });
                    let twice = format_source_with_config(
                        &parser,
                        &Source::inline(Path::new(name), &output),
                        &config,
                    )
                    .unwrap();
                    assert_eq!(
                        output, twice,
                        "{name} {standard:?} {keyword_case:?} aligned={align}"
                    );
                    eprintln!(
                    "{name} {standard:?} {keyword_case:?} aligned={align}: preservation and idempotency passed"
                );
                }
            }
        }
    }
}

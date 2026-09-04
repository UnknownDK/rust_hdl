use std::path::Path;
use vhdl_lang::{format_text_with_config, FormatConfig, KeywordCase, VHDLParser, VHDLStandard};

fn config(max_width: usize) -> FormatConfig {
    FormatConfig {
        max_width,
        ..FormatConfig::default()
    }
}

fn checked(source: &str, config: FormatConfig) -> String {
    checked_standard(source, config, VHDLStandard::VHDL2008)
}

fn checked_standard(source: &str, config: FormatConfig, standard: VHDLStandard) -> String {
    let parser = VHDLParser::new(standard);
    let path = Path::new("positional_aggregates.vhd");
    let output = format_text_with_config(&parser, path, source, &config)
        .unwrap_or_else(|error| panic!("{error:?}\n{source}"));
    let twice = format_text_with_config(&parser, path, &output, &config).unwrap();
    assert_eq!(output, twice, "{standard:?} {config:?}\n{source}");
    assert!(output.lines().all(|line| !line.ends_with(' ')));
    output
}

fn package(declaration: &str) -> String {
    format!("package p is type values_t is array (natural range <>) of real; {declaration} end;")
}

fn assert_width(output: &str, width: usize) {
    assert!(
        output.lines().all(|line| line.chars().count() <= width),
        "{output}"
    );
}

#[test]
fn reviewed_positional_aggregate_fixture() {
    assert_eq!(
        checked(
            include_str!("formatting/positional_aggregates.input.vhd"),
            FormatConfig {
                indent_width: 2,
                ..config(60)
            },
        ),
        include_str!("formatting/positional_aggregates.expected.vhd")
    );
}

#[test]
fn compact_scalar_aggregates_stay_inline() {
    let source = package("constant a: values_t := (-1.0, 0.0, 1.0); constant b: values_t := (LOW, work.pkg.HIGH, data'left); ");
    let output = checked(&source, config(100));
    assert!(
        output.contains("constant a: values_t := (-1.0, 0.0, 1.0);"),
        "{output}"
    );
    assert!(
        output.contains("constant b: values_t := (LOW, work.pkg.HIGH, data'left);"),
        "{output}"
    );
}

#[test]
fn scalar_rows_fill_up_to_the_configured_width() {
    let source = package("constant table: values_t := (-10.0, -9.0, -8.0, -7.0, -6.0, -5.0, -4.0, -3.0, -2.0, -1.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0); ");
    let output = checked(&source, config(55));
    assert!(output.contains("        -10.0, -9.0, -8.0, -7.0, -6.0, -5.0, -4.0,\n        -3.0, -2.0, -1.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0,\n        6.0, 7.0, 8.0, 9.0, 10.0\n"), "{output}");
    assert_width(&output, 55);
}

#[test]
fn negative_signs_stay_attached_at_width_boundaries() {
    let source = package("constant table: values_t := (-12345.0, -23456.0, -34567.0); ");
    for width in [20, 24, 30, 40] {
        let output = checked(&source, config(width));
        assert!(!output.lines().any(|line| line.trim() == "-"), "{output}");
        if width >= 30 {
            assert_width(&output, width);
        }
    }
}

#[test]
fn a_complex_value_keeps_the_aggregate_structural() {
    let source = package("constant table: values_t := (calculate(first, second, third), first + second, SIMPLE_VALUE, 10.0); ");
    let output = checked(&source, config(60));
    assert!(output.contains("        calculate(\n"), "{output}");
    assert!(
        output.contains("        first + second,\n        SIMPLE_VALUE,\n        10.0\n"),
        "{output}"
    );
    assert_width(&output, 60);
}

#[test]
fn named_and_mixed_aggregates_keep_their_existing_layout() {
    let source = package("constant named: values_t := (first => 1.0, second => 2.0, others => 0.0); constant mixed: values_t := (1.0, second => 2.0, others => 0.0); ");
    let output = checked(
        &source,
        FormatConfig {
            inline_argument_limit: 2,
            ..config(100)
        },
    );
    assert!(output.contains("constant named: values_t := (\n        first => 1.0,\n        second => 2.0,\n        others => 0.0\n    );"), "{output}");
    assert!(output.contains("constant mixed: values_t := (\n        1.0,\n        second => 2.0,\n        others => 0.0\n    );"), "{output}");
}

#[test]
fn comments_force_local_breaks_then_packing_resumes() {
    let source = package("constant table: values_t := (-10.0, -9.0, -8.0, -- first section\n-7.0, -6.0, -- second section\n-- final section\n-5.0, -4.0, -3.0, -2.0, -1.0); ");
    let output = checked(&source, config(55));
    assert!(output.contains("-8.0, -- first section\n        -7.0, -6.0, -- second section\n        -- final section\n        -5.0, -4.0, -3.0, -2.0, -1.0"), "{output}");
}

#[test]
fn nested_aggregates_choose_packing_independently() {
    let source = "package p is type row_t is array (natural range <>) of integer; type matrix_t is array (natural range <>) of row_t; constant matrix: matrix_t := ((1, 2, 3, 4, 5, 6, 7, 8), (9, 10, 11, 12, 13, 14, 15, 16), (17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32)); end;";
    let output = checked(source, config(45));
    assert!(output.contains("        (1, 2, 3, 4, 5, 6, 7, 8),\n        (9, 10, 11, 12, 13, 14, 15, 16),\n        (\n            17, 18, 19, 20, 21, 22, 23, 24,\n            25, 26, 27, 28, 29, 30, 31, 32\n        )\n"), "{output}");
    assert_width(&output, 45);
}

#[test]
fn qualified_aggregates_use_the_same_scalar_packing() {
    let source = package("constant table: values_t := values_t'(-10.0, -9.0, -8.0, -7.0, -6.0, -5.0, -4.0, -3.0, -2.0, -1.0); ");
    let output = checked(&source, config(45));
    assert!(output.contains("values_t'(\n        -10.0, -9.0, -8.0, -7.0, -6.0, -5.0,\n        -4.0, -3.0, -2.0, -1.0\n    )"), "{output}");
    assert_width(&output, 45);
}

#[test]
fn aggregate_targets_remain_structural() {
    let source = "entity e is end; architecture rtl of e is begin process begin (first_long_target, second_long_target, third_long_target, fourth_long_target) := source_value; wait; end process; end;";
    let output = checked(source, config(50));
    assert!(output.contains("        (\n            first_long_target,\n            second_long_target,\n            third_long_target,\n            fourth_long_target\n        ) := source_value;"), "{output}");
    assert_width(&output, 50);
}

#[test]
fn scalar_packing_is_independent_of_argument_limit_and_alignment() {
    let source = package("constant table: values_t := (-10.0, -9.0, -8.0, -7.0, -6.0, -5.0, -4.0, -3.0, -2.0, -1.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0); ");
    let expected = checked(&source, config(48));
    for inline_argument_limit in [0, 2, 100] {
        for align_associations in [false, true] {
            assert_eq!(
                checked(
                    &source,
                    FormatConfig {
                        inline_argument_limit,
                        align_associations,
                        ..config(48)
                    }
                ),
                expected
            );
        }
    }
}

#[test]
fn comments_and_packing_are_stable_across_settings() {
    let source = "package p is type row_t is array(natural range <>) of real; constant literals: row_t := (-10.0, -9.0, -8.0, -- row\n-7.0, -6.0, -5.0, -4.0, -3.0, -2.0, -1.0, 0.0, 1.0); constant names: row_t := (FIRST, work.pkg.SECOND, value'left, THIRD, FOURTH, FIFTH, SIXTH); constant complex: row_t := (calculate(a, b, c), a + b, 1.0); constant nested: row_t := row_t'(-4.0, -3.0, -2.0, -1.0); end;";
    for max_width in [24, 40, 60, 100] {
        for indent_width in [0, 2, 4] {
            for inline_argument_limit in [0, 2, 5] {
                for keyword_case in [KeywordCase::Lower, KeywordCase::Upper] {
                    for align_associations in [false, true] {
                        checked(
                            source,
                            FormatConfig {
                                max_width,
                                indent_width,
                                inline_argument_limit,
                                keyword_case,
                                align_associations,
                                ..FormatConfig::default()
                            },
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn compatible_aggregates_match_across_language_versions() {
    let source = include_str!("formatting/positional_aggregates.input.vhd");
    let config = FormatConfig {
        indent_width: 2,
        ..config(60)
    };
    let expected = checked_standard(source, config, VHDLStandard::VHDL1993);
    for standard in [VHDLStandard::VHDL2008, VHDLStandard::VHDL2019] {
        assert_eq!(checked_standard(source, config, standard), expected);
    }
}

#[test]
fn disabled_aggregates_remain_verbatim() {
    let raw = "-- vhdl_ls off\n  constant raw : values_t := ( 1.0,2.0,  3.0 );\n-- vhdl_ls on\n";
    let source = format!("package p is type values_t is array (natural range <>) of real;\n{raw}constant table: values_t := (-10.0, -9.0, -8.0, -7.0, -6.0); end;");
    let output = checked(&source, config(40));
    assert!(output.contains(raw), "{output}");
    assert!(
        output.contains("constant table: values_t := (\n"),
        "{output}"
    );
}

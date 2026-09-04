use std::path::Path;
use vhdl_lang::{format_text_with_config, FormatConfig, KeywordCase, VHDLParser, VHDLStandard};

fn checked(input: &str, config: FormatConfig) -> String {
    let parser = VHDLParser::new(VHDLStandard::VHDL2008);
    let path = Path::new("readability.vhd");
    let once = format_text_with_config(&parser, path, input, &config)
        .unwrap_or_else(|error| panic!("{error:?}\n{input}"));
    let twice = format_text_with_config(&parser, path, &once, &config).unwrap();
    assert_eq!(once, twice, "not idempotent: {config:?}\n{input}");
    once
}

fn process(body: &str) -> String {
    format!("entity e is end; architecture rtl of e is begin process(all) begin {body} end process; end;")
}

#[test]
fn reviewed_readability_fixture() {
    let input = include_str!("formatting/readability.input.vhd");
    let config = FormatConfig {
        max_width: 60,
        align_associations: true,
        ..FormatConfig::default()
    };
    assert_eq!(
        checked(input, config),
        include_str!("formatting/readability.expected.vhd")
    );
}

#[test]
fn readability_rules_work_across_language_versions() {
    let input = process("if valid = '1' and ready = '1' and enabled = '1' then x <= (a => 1, b => 2, c => 3); end if;").replace("process(all)", "process");
    let path = Path::new("versions.vhd");
    let config = FormatConfig {
        max_width: 50,
        align_associations: true,
        ..FormatConfig::default()
    };
    let mut expected = None;
    for standard in [
        VHDLStandard::VHDL1993,
        VHDLStandard::VHDL2008,
        VHDLStandard::VHDL2019,
    ] {
        let parser = VHDLParser::new(standard);
        let output = format_text_with_config(&parser, path, &input, &config).unwrap();
        assert_eq!(
            output,
            format_text_with_config(&parser, path, &output, &config).unwrap()
        );
        assert_eq!(expected.get_or_insert_with(|| output.clone()), &output);
    }
}

#[test]
fn wide_aggregate_rows_disable_padding_instead_of_exceeding_width() {
    let source = process("x <= (a => a_long_value, longer_name => 0, c => 1);");
    let output = checked(
        &source,
        FormatConfig {
            max_width: 37,
            align_associations: true,
            ..FormatConfig::default()
        },
    );
    assert!(
        output.contains(
            "            a => a_long_value,\n            longer_name => 0,\n            c => 1"
        ),
        "{output}"
    );
    assert!(output.lines().all(|line| line.len() <= 37), "{output}");
}

#[test]
fn logical_conditions_keep_comparisons_together_and_then_on_its_own_line() {
    let source = process("if input_valid = '1' and output_ready = '1' and transfer_enabled = '1' then null; elsif a = b then null; end if;");
    let output = checked(
        &source,
        FormatConfig {
            max_width: 60,
            ..FormatConfig::default()
        },
    );
    assert!(output.contains("        if input_valid = '1'\n            and output_ready = '1'\n            and transfer_enabled = '1'\n        then\n            null;\n        elsif a = b then\n"), "{output}");
    let wide = checked(
        &source,
        FormatConfig {
            max_width: 120,
            ..FormatConfig::default()
        },
    );
    assert!(
        wide.contains(
            "if input_valid = '1' and output_ready = '1' and transfer_enabled = '1' then\n"
        ),
        "{wide}"
    );
}

#[test]
fn arithmetic_and_concatenation_wrap_between_complete_operands() {
    let source = process("result <= first_operand * second_operand + third_operand * fourth_operand; result <= header_word & payload_word & checksum_word;");
    let output = checked(
        &source,
        FormatConfig {
            max_width: 60,
            ..FormatConfig::default()
        },
    );
    assert!(output.contains("        result <= first_operand * second_operand\n            + third_operand * fourth_operand;"), "{output}");
    assert!(output.contains("        result <= header_word\n            & payload_word\n            & checksum_word;"), "{output}");
}

#[test]
fn conditional_assignment_alternatives_remain_together() {
    let source = process("result <= data_a when select_a = '1' else data_b when select_b = '1' else (others => '0');");
    let output = checked(
        &source,
        FormatConfig {
            max_width: 60,
            ..FormatConfig::default()
        },
    );
    assert!(output.contains("        result <= data_a when select_a = '1'\n            else data_b when select_b = '1'\n            else (others => '0');"), "{output}");
}

#[test]
fn named_aggregates_use_the_shared_limit_and_optional_alignment() {
    let source = process("header <= (valid => '1', opcode => OP_WRITE, length => payload_length);");
    for inline_argument_limit in [0, 1, 2, 3, 100] {
        for align_associations in [false, true] {
            let output = checked(
                &source,
                FormatConfig {
                    max_width: 120,
                    inline_argument_limit,
                    align_associations,
                    ..FormatConfig::default()
                },
            );
            if inline_argument_limit >= 3 {
                assert!(
                    output.contains(
                        "header <= (valid => '1', opcode => OP_WRITE, length => payload_length);"
                    ),
                    "{output}"
                );
            } else {
                let gap = if align_associations { "  " } else { " " };
                assert!(output.contains(&format!("        header <= (\n            valid{gap}=> '1',\n            opcode => OP_WRITE,\n            length => payload_length\n        );")), "{output}");
            }
        }
    }
    let short = process("x <= (a => 1, b => 2); y <= (others => '0'); z <= (1, 2, 3, 4);");
    let defaults = checked(&short, FormatConfig::default());
    assert!(defaults.contains("x <= (a => 1, b => 2);"));
    assert!(defaults.contains("y <= (others => '0');"));
    let expanded = checked(
        &short,
        FormatConfig {
            inline_argument_limit: 0,
            ..FormatConfig::default()
        },
    );
    assert!(
        expanded.contains("y <= (\n            others => '0'\n        );"),
        "{expanded}"
    );
    assert!(expanded.contains("z <= (1, 2, 3, 4);"), "{expanded}");
}

#[test]
fn nested_aggregates_choose_independent_layouts_and_alignment_runs() {
    let source = process(
        "header <= (a => (x => 1, y => 2), nested => (a => 1, longer => 2, c => 3), tail => 0);",
    );
    let output = checked(
        &source,
        FormatConfig {
            align_associations: true,
            ..FormatConfig::default()
        },
    );
    assert!(output.contains("            a => (x => 1, y => 2),\n            nested => (\n                a      => 1,\n                longer => 2,\n                c      => 3\n            ),\n            tail => 0"), "{output}");
}

#[test]
fn aggregate_alignment_respects_comments_blank_lines_and_positional_items() {
    let source = process("x <= (0, a => 1, longer => 2,\n\n-- section\nb => 3, longest => 4, c => 5, -- keep\nd => 6, extra => 7);");
    let output = checked(
        &source,
        FormatConfig {
            align_associations: true,
            ..FormatConfig::default()
        },
    );
    assert!(output.contains("            0,\n            a      => 1,\n            longer => 2,\n\n            -- section\n            b       => 3,\n            longest => 4,\n            c => 5, -- keep\n            d     => 6,\n            extra => 7"), "{output}");
}

#[test]
fn structural_blank_lines_keep_comments_attached_and_simple_rows_together() {
    let source = "library ieee; use ieee.std_logic_1164.all; -- imports\n-- entity docs\nentity e is end; architecture rtl of e is constant a: natural := 1;\n-- function docs\nfunction f return natural is begin return a; end; -- function end\nconstant b: natural := 2; constant c: natural := 3;\n\n\n\n-- declaration group\nsignal x: natural; begin process(all) begin x <= f; end process; -- process end\n-- output process\nprocess(all) begin x <= b; end process; x <= b; x <= c; end;";
    let output = checked(source, FormatConfig::default());
    assert!(
        output.contains("use ieee.std_logic_1164.all; -- imports\n\n-- entity docs\nentity e is"),
        "{output}"
    );
    assert!(
        output.contains("constant a: natural := 1;\n\n    -- function docs\n    function f"),
        "{output}"
    );
    assert!(output.contains("end; -- function end\n\n    constant b: natural := 2;\n    constant c: natural := 3;\n\n    -- declaration group"), "{output}");
    assert!(
        output.contains("end process; -- process end\n\n    -- output process\n    process(all)"),
        "{output}"
    );
    assert!(
        output.contains("end process;\n\n    x <= b;\n    x <= c;"),
        "{output}"
    );
    assert!(!output.contains("\n\n\n"), "{output}");
}

#[test]
fn context_separation_covers_all_design_units_but_not_context_bodies() {
    for unit in [
        "entity e is end;",
        "architecture rtl of e is begin end;",
        "package p is end;",
        "package body p is end;",
        "package p is new work.generic_pkg generic map (1);",
        "configuration cfg of e is for rtl end for; end;",
    ] {
        let source = format!("library lib; use lib.pkg.all;\n-- unit docs\n{unit}");
        let output = checked(&source, FormatConfig::default());
        assert!(
            output.starts_with("library lib;\nuse lib.pkg.all;\n\n-- unit docs\n"),
            "{output}"
        );
    }
    let context = checked(
        "context c is library lib; use lib.pkg.all; end context;",
        FormatConfig::default(),
    );
    assert_eq!(
        context,
        "context c is\n    library lib;\n    use lib.pkg.all;\nend context;\n"
    );
}

#[test]
fn readability_rules_preserve_comments_parentheses_and_tokens_across_options() {
    let source = process("if (a = b or c = d) and e /= f then x <= t'(a => (1, 2, 3), b => (x => 0, y => 1), others => 0); elsif enabled(a, b, c) then x <= (0, 1, others => 2); end if; if first = -- compare\nsecond and -- logical\nthird = fourth then -- body\nnull; end if; x <= (a | b => 1, -- choices\nc => 2,\n/* anchor */ others => 3); x <= (a + b) * (c - d) & e;\n-- vhdl_ls off\ninvalid € \"\n-- vhdl_ls on\nx <= a;");
    for max_width in [20, 30, 40, 60, 100] {
        for indent_width in [0, 2, 4] {
            for inline_argument_limit in [0, 2, 5] {
                for align_associations in [false, true] {
                    for keyword_case in [KeywordCase::Lower, KeywordCase::Upper] {
                        let config = FormatConfig {
                            max_width,
                            indent_width,
                            inline_argument_limit,
                            align_associations,
                            keyword_case,
                            ..FormatConfig::default()
                        };
                        let output = checked(&source, config);
                        // Unmodified comments and disabled text may exceed width.
                        for line in output.lines().filter(|line| {
                            !line.contains("--") && !line.contains("/*") && !line.contains('€')
                        }) {
                            // Very narrow widths can be exceeded by fixed
                            // structural headers or indentation plus a token.
                            if max_width >= 40 {
                                assert!(line.chars().count() <= max_width, "{config:?}\n{output}");
                            }
                        }
                    }
                }
            }
        }
    }
}

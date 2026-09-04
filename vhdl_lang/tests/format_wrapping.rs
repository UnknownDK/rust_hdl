use std::path::Path;
use vhdl_lang::{
    format_source_with_config, FormatConfig, KeywordCase, Source, VHDLParser, VHDLStandard,
};

fn checked(input: &str, config: FormatConfig, standard: VHDLStandard) -> String {
    let parser = VHDLParser::new(standard);
    let path = Path::new("wrapping.vhd");
    // The public API checks parsing, exact non-keyword spelling and token order,
    // plus comment text/order and the gap between tokens anchoring each comment.
    let once = format_source_with_config(&parser, &Source::inline(path, input), &config)
        .unwrap_or_else(|error| panic!("{error:?}\n{input}"));
    let twice = format_source_with_config(&parser, &Source::inline(path, &once), &config).unwrap();
    assert_eq!(once, twice, "not idempotent at {config:?}\n{input}");
    let mut diagnostics = Vec::new();
    parser.parse_design_source(&Source::inline(path, &once), &mut diagnostics);
    assert!(
        diagnostics.is_empty(),
        "formatted source does not parse: {diagnostics:?}\n{once}"
    );
    once
}

fn config(width: usize) -> FormatConfig {
    FormatConfig {
        max_width: width,
        ..FormatConfig::default()
    }
}

#[test]
fn keyword_case_changes_only_reserved_words() {
    let input = r#"PACKAGE Mixed IS
constant FooBar: string := "AND Entity -- vhdl_ls off";
constant Bits: bit_vector := X"aF";
constant Number: real := 1.2E3;
constant Letter: character := 'Z';
constant Flag: boolean := NOT (true AND false);
function "AND" (A, B: bit) return bit;
constant \MiXeD Name\: integer := 16#Ab#;
-- ENTITY FooBar æ € 😀
END PACKAGE;"#;
    for keyword_case in [KeywordCase::Lower, KeywordCase::Upper] {
        let output = checked(
            input,
            FormatConfig {
                keyword_case,
                ..config(60)
            },
            VHDLStandard::VHDL2008,
        );
        for preserved in [
            "Mixed",
            "FooBar",
            "\"AND Entity -- vhdl_ls off\"",
            "X\"aF\"",
            "1.2E3",
            "'Z'",
            "\"AND\"",
            "\\MiXeD Name\\",
            "16#Ab#",
            "-- ENTITY FooBar æ € 😀",
        ] {
            assert!(output.contains(preserved), "missing {preserved}:\n{output}");
        }
        assert!(output.starts_with(if keyword_case == KeywordCase::Upper {
            "PACKAGE Mixed IS"
        } else {
            "package Mixed is"
        }));
        assert!(output.contains(if keyword_case == KeywordCase::Upper {
            "NOT (true AND false)"
        } else {
            "not (true and false)"
        }));
    }
}

#[test]
fn nested_calls_aggregates_and_statements_wrap() {
    let input = r#"entity demo is end entity;
architecture rtl of demo is
signal first_signal, second_signal: integer_vector(0 to 7) := (others => 0);
subtype index_type is integer range first_bound + extra_bound to last_bound - extra_bound;
function compute(first_argument: integer; second_argument: integer) return integer;
begin
child: entity work.demo generic map (size => compute(first_argument, second_argument)) port map (a => first_signal, b => second_signal);
process
variable result: integer;
begin
result := calculate(first_operand, nested(second_operand, third_operand), enable => condition);
result := first_operand + second_operand + third_operand * fourth_operand;
first_signal <= integer_vector'(first_operand, second_operand, others => third_operand);
result := first_operand when first_condition else second_operand when second_condition else third_operand;
with selection_value select result := first_operand when 0, second_operand when others;
assert first_condition and second_condition report "a message" & "another message" severity failure;
report "a message" & "another message" severity warning;
wait on first_signal, second_signal until first_condition and second_condition for long_delay + more_delay;
return calculate(first_operand, second_operand);
end process;
end architecture;"#;
    for width in [40, 60, 100] {
        let output = checked(input, config(width), VHDLStandard::VHDL2008);
        for line in output.lines() {
            assert!(
                line.chars().count() <= width,
                "over width {width}: {line}\n{output}"
            );
            assert!(!line.ends_with(' '), "trailing whitespace: {line}");
        }
        assert!(output.contains("calculate(\n"));
    }
}

#[test]
fn comments_at_every_call_break_remain_anchored() {
    let input = "entity demo is end; architecture rtl of demo is begin process begin\nresult := calculate( -- opening\n-- first\na, -- comma\n/* before second */ b,\nnamed => -- arrow\nnested(c, -- nested comma\nd) -- nested closing\n); -- final\nend process; end;\n-- eof æ € 😀\n";
    for width in [30, 100] {
        checked(input, config(width), VHDLStandard::VHDL2008);
    }
}

#[test]
fn disabled_regions_are_lossless_and_enabled_sections_are_formatted() {
    let raw = "  -- vhdl_ls off\ninvalid € VHDL \"   \n  -- vhdl_ls on\n";
    let input = format!("ENTITY Foo IS\n{raw}END ENTITY;\n");
    let output = checked(
        &input,
        FormatConfig {
            keyword_case: KeywordCase::Upper,
            ..config(40)
        },
        VHDLStandard::VHDL2008,
    );
    assert_eq!(output, format!("ENTITY Foo IS\n{raw}END ENTITY;\n"));
    let output = checked(&input, config(40), VHDLStandard::VHDL2008);
    assert_eq!(output, format!("entity Foo is\n{raw}end entity;\n"));
    for source in [
        "ENTITY Foo IS END;\n--vhdl_ls off\ninvalid, without an on marker",
        "--vhdl_ls off\nanything\n--vhdl_ls on\nENTITY Foo IS END;",
        "ENTITY Foo IS\n--vhdl_ls off\n--vhdl_ls on\nEND;",
        "ENTITY Foo IS --vhdl_ls off\nanything\n--vhdl_ls on\nEND;",
        "ENTITY Foo IS\n/* vhdl_ls off */\nanything\n/* vhdl_ls on */\nEND;",
    ] {
        checked(source, config(40), VHDLStandard::VHDL2008);
    }
}

#[test]
fn whitespace_variants_are_canonical_across_standards_and_options() {
    let tokens = [
        "entity",
        "Foo",
        "is",
        "end",
        ";",
        "architecture",
        "rtl",
        "of",
        "Foo",
        "is",
        "begin",
        "process",
        "begin",
        "x",
        ":=",
        "f",
        "(",
        "a",
        ",",
        "b",
        ")",
        "+",
        "c",
        ";",
        "wait",
        ";",
        "end",
        "process",
        ";",
        "end",
        ";",
    ];
    for standard in [
        VHDLStandard::VHDL1993,
        VHDLStandard::VHDL2008,
        VHDLStandard::VHDL2019,
    ] {
        for width in [24, 40, 100] {
            for keyword_case in [KeywordCase::Lower, KeywordCase::Upper] {
                let config = FormatConfig {
                    keyword_case,
                    indent_width: 2,
                    ..config(width)
                };
                let canonical = checked(&tokens.join(" "), config, standard);
                for seed in 0..32u64 {
                    let mut state = seed + 1;
                    let mut input = String::new();
                    for token in tokens {
                        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                        input.push_str([" ", "\t", "\n", " \n  "][(state >> 32) as usize % 4]);
                        input.push_str(token);
                    }
                    assert_eq!(checked(&input, config, standard), canonical);
                }
            }
        }
    }
}

#[test]
fn width_boundary_uses_the_closing_semicolon() {
    let input = "entity e is end; architecture rtl of e is begin f(aaaa, bbbb); end;";
    for width in [17, 18, 19] {
        let output = checked(input, config(width), VHDLStandard::VHDL1993);
        assert_eq!(output.contains("    f(aaaa, bbbb);"), width >= 18);
    }
}

#[test]
fn vhdl_2019_views_are_supported() {
    let input = "package p is type pair is record a: bit; b: bit; end record; view pair_view of pair is a: in; b: out; end view; end package;";
    checked(input, config(40), VHDLStandard::VHDL2019);
}

#[test]
fn nested_layouts_are_stable_at_many_widths_and_indents() {
    let input = "entity e is end; architecture rtl of e is begin process begin x := f(a, g(b, h(c, d)), (others => z)) + aa + bb; end process; end;";
    for indent_width in [0, 2, 4] {
        for width in 24..=120 {
            checked(
                input,
                FormatConfig {
                    indent_width,
                    ..config(width)
                },
                VHDLStandard::VHDL2008,
            );
        }
    }
}

#[test]
fn line_comments_at_each_token_gap_are_preserved() {
    let tokens = [
        "entity",
        "e",
        "is",
        "end",
        ";",
        "architecture",
        "rtl",
        "of",
        "e",
        "is",
        "begin",
        "process",
        "begin",
        "x",
        ":=",
        "f",
        "(",
        "a",
        ",",
        "b",
        ")",
        "+",
        "c",
        ";",
        "end",
        "process",
        ";",
        "end",
        ";",
    ];
    for gap in 0..=tokens.len() {
        let input = format!(
            "{} -- anchor æ € 😀\n{}",
            tokens[..gap].join(" "),
            tokens[gap..].join(" ")
        );
        for width in [32, 100] {
            checked(&input, config(width), VHDLStandard::VHDL1993);
        }
    }
}

#[test]
fn disabled_region_diagnostics_use_original_source_positions() {
    let input = "entity e is\n--vhdl_ls off\ninvalid\nmore invalid\n--vhdl_ls on\nend error e;";
    let parser = VHDLParser::new(VHDLStandard::VHDL2008);
    let source = Source::inline(Path::new("original.vhd"), input);
    let error = format_source_with_config(&parser, &source, &config(40)).unwrap_err();
    let vhdl_lang::FormatError::InputDiagnostics(diagnostics) = error else {
        panic!("{error:?}");
    };
    assert!(!diagnostics.is_empty());
    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.pos.start().line == 5));
    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.pos.source == source));
}

#[test]
fn long_declaration_names_and_associations_have_legal_breaks() {
    let input = "entity e is end; architecture rtl of e is
subtype very_long_subtype_identifier is very_long_base_type_identifier;
signal first_long_signal_name, second_long_signal_name, third_long_signal_name: very_long_base_type_identifier;
function very_long_function_identifier return very_long_base_type_identifier;
begin
very_long_instance_label: entity work.very_long_component_identifier
port map (very_long_formal_identifier => very_long_actual_identifier);
x <= (very_long_choice_identifier => very_long_actual_identifier);
end;";
    for width in [50, 60, 100] {
        let output = checked(input, config(width), VHDLStandard::VHDL2008);
        for line in output.lines() {
            assert!(line.len() <= width, "width {width}: {line}\n{output}");
        }
    }
}

use std::path::Path;
use vhdl_lang::{format_text_with_config, FormatConfig, KeywordCase, VHDLParser, VHDLStandard};

fn checked(input: &str, config: FormatConfig) -> String {
    checked_standard(input, config, VHDLStandard::VHDL2008)
}

fn checked_standard(input: &str, config: FormatConfig, standard: VHDLStandard) -> String {
    let parser = VHDLParser::new(standard);
    let path = Path::new("layout_consistency.vhd");
    let output = format_text_with_config(&parser, path, input, &config)
        .unwrap_or_else(|error| panic!("{error:?}\n{input}"));
    let twice = format_text_with_config(&parser, path, &output, &config).unwrap();
    assert_eq!(output, twice, "{standard:?} {config:?}\n{input}");
    output
}

fn config(width: usize) -> FormatConfig {
    FormatConfig {
        max_width: width,
        ..FormatConfig::default()
    }
}

fn process(body: &str) -> String {
    format!(
        "entity e is end; architecture rtl of e is begin process begin {body} end process; end;"
    )
}

#[test]
fn reviewed_layout_consistency_fixture() {
    let input = include_str!("formatting/layout_consistency.input.vhd");
    assert_eq!(
        checked(
            input,
            FormatConfig {
                align_declarations: true,
                ..config(60)
            }
        ),
        include_str!("formatting/layout_consistency.expected.vhd")
    );
}

#[test]
fn grouped_ports_keep_siblings_level_and_move_the_complete_type() {
    let input = "entity e is port(first_input_signal, second_input_signal, third_input_signal: in std_logic_vector(31 downto 0)); end;";
    let output = checked(
        input,
        FormatConfig {
            align_declarations: true,
            ..config(60)
        },
    );
    assert_eq!(output, "entity e is\n    port (\n        first_input_signal,\n        second_input_signal,\n        third_input_signal :\n            in std_logic_vector(31 downto 0)\n    );\nend;\n");
}

#[test]
fn function_parameters_records_and_objects_prefer_whole_types() {
    let input = "package p is function f(first_parameter, second_parameter, third_parameter: std_logic_vector(31 downto 0)) return bit; type r is record first_field, second_field, third_field: std_logic_vector(31 downto 0); end record; signal very_long_signal_name: std_logic_vector(31 downto 0) := (others => '0'); end;";
    let output = checked(input, config(50));
    assert!(output.contains("        first_parameter,\n        second_parameter,\n        third_parameter:\n            std_logic_vector(31 downto 0)"), "{output}");
    assert!(output.contains("        first_field, second_field, third_field:\n            std_logic_vector(31 downto 0);"), "{output}");
    assert!(
        output.contains("signal very_long_signal_name:\n        std_logic_vector(31 downto 0)"),
        "{output}"
    );
    let compact = checked(
        "package p is signal a, b: bit; function f(a, b: bit) return bit; end;",
        config(100),
    );
    assert!(compact.contains("signal a, b: bit;"));
    assert!(compact.contains("function f(a, b: bit) return bit;"));
}

#[test]
fn keyword_prefixed_names_keep_continuation_indentation() {
    let source = "package p is signal first_long_signal, second_long_signal, third_long_signal: std_logic_vector(31 downto 0); procedure read_data(file first_file, second_file: file_of_vectors); end;";
    let output = checked(source, config(45));
    assert!(output.contains("    signal first_long_signal,\n        second_long_signal,\n        third_long_signal:"), "{output}");
    assert!(output.contains("std_logic_vector(31 downto 0)"), "{output}");
    assert!(output.contains("file first_file,"), "{output}");
    assert!(output.lines().all(|line| line.len() <= 45), "{output}");
}

#[test]
fn narrow_types_can_still_wrap_internally() {
    let input = "entity e is port(a: in std_logic_vector(first_bound + offset downto last_bound - offset)); end;";
    let output = checked(input, config(36));
    assert!(output.contains("std_logic_vector(\n"), "{output}");
    assert!(output.lines().all(|line| line.len() <= 36), "{output}");
}

#[test]
fn expanded_loops_put_loop_on_its_own_line() {
    let output = checked(&process("while first_condition and second_condition and third_condition loop null; end loop; for index in first_index + offset to last_index - offset loop null; end loop; while a loop null; end loop; for i in 0 to 1 loop null; end loop; loop wait; end loop;"), config(60));
    assert!(output.contains("        while first_condition\n            and second_condition\n            and third_condition\n        loop\n"), "{output}");
    assert!(output.contains("        for index in first_index + offset\n            to last_index - offset\n        loop\n"), "{output}");
    assert!(output.contains("while a loop\n"));
    assert!(output.contains("for i in 0 to 1 loop\n"));
    assert!(output.contains("        loop\n            wait;\n        end loop;"));
}

#[test]
fn expanded_generate_headers_keep_their_labels_and_boundaries() {
    let input = "entity e is end; architecture rtl of e is begin g: if FEATURE_A_ENABLED and FEATURE_B_ENABLED and FEATURE_C_ENABLED generate x <= a; elsif alt: OTHER_A_ENABLED and OTHER_B_ENABLED and OTHER_C_ENABLED generate x <= b; else fallback: generate x <= c; end generate g; f: for i in first_index + offset to last_index - offset generate x <= a; end generate; short: if enabled generate x <= a; end generate; end;";
    let output = checked(input, config(60));
    assert!(output.contains("    g: if FEATURE_A_ENABLED\n        and FEATURE_B_ENABLED\n        and FEATURE_C_ENABLED\n    generate\n"), "{output}");
    assert!(output.contains("    elsif alt: OTHER_A_ENABLED\n        and OTHER_B_ENABLED\n        and OTHER_C_ENABLED\n    generate\n"), "{output}");
    assert!(output.contains("    else fallback: generate\n"));
    assert!(
        output.contains(
            "    f: for i in first_index + offset to last_index - offset\n    generate\n"
        ),
        "{output}"
    );
    assert!(output.contains("short: if enabled generate\n"));
}

#[test]
fn selected_assignments_put_the_target_below_the_selector() {
    let input = "entity e is end; architecture rtl of e is begin with selection_value select result <= first_value when first_choice, second_value when second_choice, default_value when others; end;";
    let output = checked(input, config(60));
    assert!(output.contains("    with selection_value select\n        result <= first_value when first_choice,\n            second_value when second_choice,\n            default_value when others;"), "{output}");
    let compact = checked("entity e is end; architecture rtl of e is begin with s select x <= a when 0, b when others; end;", config(100));
    assert!(
        compact.contains("with s select x <= a when 0, b when others;"),
        "{compact}"
    );
}

#[test]
fn selected_variable_signal_and_force_assignments_preserve_modifiers() {
    for assignment in ["x :=", "x <=", "x <= transport", "x <= force in"] {
        let source = process(&format!("with selection_value select? {assignment} first_value when first_choice, second_value when others;"));
        let output = checked(&source, config(50));
        assert!(
            output.contains("        with selection_value select?\n"),
            "{output}"
        );
        assert!(output.contains(assignment), "{output}");
    }
    for prefix in ["", "postponed "] {
        let input = format!("entity e is end; architecture rtl of e is begin {prefix}with selection_value select? x <= reject delay_time inertial first_value when first_choice, second_value when others; end;");
        let output = checked(&input, config(60));
        assert!(
            output.contains(&format!("{prefix}with selection_value select?\n")),
            "{output}"
        );
        assert!(output.contains("reject"));
        assert!(output.contains("inertial"));
    }
}

#[test]
fn long_choices_wrap_at_bars_in_cases_and_selected_assignments() {
    let choices = "FIRST_CHOICE | SECOND_CHOICE | THIRD_CHOICE";
    let source = process(&format!("case mode is when {choices} => null; when others => null; end case; with mode select x := value when {choices}, fallback when others;"));
    let output = checked(&source, config(45));
    assert!(
        output.contains(
            "when FIRST_CHOICE\n                | SECOND_CHOICE\n                | THIRD_CHOICE =>"
        ),
        "{output}"
    );
    assert!(output.contains("| SECOND_CHOICE"), "{output}");
    assert!(output.lines().all(|line| line.len() <= 45), "{output}");
    let generate = format!("entity e is end; architecture rtl of e is begin g: case selected_mode generate when alt: {choices} => x <= a; when others => x <= b; end generate; end;");
    let output = checked(&generate, config(45));
    assert!(
        output.contains(
            "when alt: FIRST_CHOICE\n            | SECOND_CHOICE\n            | THIRD_CHOICE =>"
        ),
        "{output}"
    );
    assert!(output.lines().all(|line| line.len() <= 45), "{output}");
}

#[test]
fn comments_and_nested_layouts_remain_lossless_across_options() {
    let input = "entity e is port(a, -- first\nb, c: /* type */ in std_logic_vector(15 downto 0)); end; architecture rtl of e is begin g: if ready and -- guard\nenabled generate with mode select x <= a when first_choice | -- choice\nsecond_choice, b when others; end generate; process(clk) variable x: std_logic_vector(15 downto 0); begin outer_loop: while a = b and b = c loop for i in 0 to 7 loop case mode is when first_choice | -- case\nsecond_choice => null; when others => null; end case; end loop; end loop; end process; end;";
    for max_width in [28, 40, 60, 100] {
        for indent_width in [0, 2, 4] {
            for inline_argument_limit in [0, 2, 5] {
                for align_declarations in [false, true] {
                    for keyword_case in [KeywordCase::Lower, KeywordCase::Upper] {
                        checked(
                            input,
                            FormatConfig {
                                max_width,
                                indent_width,
                                inline_argument_limit,
                                align_declarations,
                                keyword_case,
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
fn layouts_are_identical_across_supported_standards() {
    let input = "entity e is port(first_signal, second_signal, third_signal: in bit_vector(15 downto 0)); end; architecture rtl of e is begin g: for i in first_bound to last_bound generate with mode select x <= value when first_choice | second_choice | third_choice, fallback when others; end generate; process begin while ready and enabled and available loop null; end loop; end process; end;";
    let mut expected = None;
    for standard in [
        VHDLStandard::VHDL1993,
        VHDLStandard::VHDL2008,
        VHDLStandard::VHDL2019,
    ] {
        let output = checked_standard(input, config(45), standard);
        assert_eq!(expected.get_or_insert_with(|| output.clone()), &output);
    }
}

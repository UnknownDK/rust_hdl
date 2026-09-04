use std::path::Path;
use vhdl_lang::{format_text_with_config, FormatConfig, KeywordCase, VHDLParser, VHDLStandard};

fn config(width: usize) -> FormatConfig {
    FormatConfig {
        max_width: width,
        ..FormatConfig::default()
    }
}

fn checked(input: &str, config: FormatConfig) -> String {
    checked_standard(input, config, VHDLStandard::VHDL2008)
}

fn checked_standard(input: &str, config: FormatConfig, standard: VHDLStandard) -> String {
    let parser = VHDLParser::new(standard);
    let path = Path::new("types_and_waveforms.vhd");
    // The public API verifies tokens and comment anchors as well as parsing.
    let output = format_text_with_config(&parser, path, input, &config)
        .unwrap_or_else(|error| panic!("{error:?}\n{input}"));
    let twice = format_text_with_config(&parser, path, &output, &config).unwrap();
    assert_eq!(output, twice, "{standard:?} {config:?}\n{input}");
    assert!(output.lines().all(|line| !line.ends_with(' ')));
    output
}

fn architecture(body: &str) -> String {
    format!("entity e is end; architecture rtl of e is begin {body} end;")
}

fn assert_width(output: &str, width: usize) {
    assert!(
        output.lines().all(|line| line.chars().count() <= width),
        "{output}"
    );
}

#[test]
fn reviewed_types_and_waveforms_fixture() {
    assert_eq!(
        checked(
            include_str!("formatting/types_and_waveforms.input.vhd"),
            config(60)
        ),
        include_str!("formatting/types_and_waveforms.expected.vhd")
    );
}

#[test]
fn short_types_stay_inline_regardless_of_argument_limit() {
    let source = "package p is type state_t is (IDLE, RUNNING, DONE); type memory_t is array (0 to 255) of byte_t; type grid_t is array (0 to 1, 0 to 2, 0 to 3) of bit; end;";
    for inline_argument_limit in [0, 2, 5] {
        let output = checked(
            source,
            FormatConfig {
                inline_argument_limit,
                ..config(100)
            },
        );
        assert_eq!(output, "package p is\n    type state_t is (IDLE, RUNNING, DONE);\n    type memory_t is array (0 to 255) of byte_t;\n    type grid_t is array (0 to 1, 0 to 2, 0 to 3) of bit;\nend;\n");
    }
}

#[test]
fn enum_width_boundary_reserves_closing_delimiter_and_semicolon() {
    let source = "package p is type state_t is (IDLE, RUNNING, DONE); end;";
    let compact = checked(source, config(42));
    assert!(
        compact.contains("    type state_t is (IDLE, RUNNING, DONE);"),
        "{compact}"
    );
    let expanded = checked(source, config(41));
    assert_eq!(expanded, "package p is\n    type state_t is (\n        IDLE,\n        RUNNING,\n        DONE\n    );\nend;\n");
    assert_width(&expanded, 41);
}

#[test]
fn enum_literals_and_comments_are_preserved() {
    let source = "package p is type alphabet is ( -- opening\n'A', -- letter\n-- next\n'Z', \\Mixed Name\\, last_value -- final\n); end;";
    let output = checked(
        source,
        FormatConfig {
            keyword_case: KeywordCase::Upper,
            ..config(40)
        },
    );
    assert!(output.contains("'A', -- letter"), "{output}");
    assert!(
        output.contains("'Z',\n        \\Mixed Name\\,\n        last_value -- final"),
        "{output}"
    );
}

#[test]
fn arrays_move_whole_element_types_before_splitting_constraints() {
    let source = "package p is type matrix_t is array (natural range <>, natural range <>) of std_logic_vector(DATA_WIDTH - 1 downto 0); end;";
    let output = checked(source, config(50));
    assert_eq!(output, "package p is\n    type matrix_t is array (\n        natural range <>,\n        natural range <>\n    ) of\n        std_logic_vector(DATA_WIDTH - 1 downto 0);\nend;\n");
    let narrow = checked(source, config(40));
    assert!(narrow.contains("std_logic_vector(\n"), "{narrow}");
    assert_width(&narrow, 40);
}

#[test]
fn constrained_array_dimensions_wrap_independently() {
    let source = "package p is type matrix_t is array (first_index + offset to last_index - offset, 7 downto 0, data'range) of bit; end;";
    let output = checked(source, config(40));
    assert!(output.contains("        first_index + offset\n            to last_index - offset,\n        7 downto 0,\n        data'range\n    ) of bit;"), "{output}");
    assert_width(&output, 40);
}

#[test]
fn short_waveforms_and_delay_mechanisms_stay_inline() {
    let body = "ready <= '1' after 10 ns; x <= '0' after 1 ns, '1' after 2 ns; x <= transport '1' after 1 ns; x <= reject 1 ns inertial '1' after 2 ns; x <= unaffected;";
    for body in [
        body.to_string(),
        format!("process begin {body} wait; end process;"),
    ] {
        let output = checked(&architecture(&body), config(100));
        for statement in [
            "ready <= '1' after 10 ns;",
            "x <= '0' after 1 ns, '1' after 2 ns;",
            "x <= transport '1' after 1 ns;",
            "x <= reject 1 ns inertial '1' after 2 ns;",
            "x <= unaffected;",
        ] {
            assert!(output.contains(statement), "{output}");
        }
    }
}

#[test]
fn expanded_waveforms_share_concurrent_and_sequential_layouts() {
    let body = "output_signal <= first_long_value after first_long_delay, second_long_value after second_long_delay;";
    for (body, indent) in [
        (body.to_string(), 4),
        (format!("process begin {body} wait; end process;"), 8),
    ] {
        let output = checked(&architecture(&body), config(60));
        let prefix = " ".repeat(indent);
        assert!(output.contains(&format!("{prefix}output_signal <=\n{prefix}    first_long_value after first_long_delay,\n{prefix}    second_long_value after second_long_delay;")), "{output}");
        assert_width(&output, 60);
    }
}

#[test]
fn long_waveform_elements_break_before_after_not_after_it() {
    let source = architecture("output_signal <= calculate_output(input_data, settings) after PROPAGATION_DELAY; x <= value after BASE_DELAY + EXTRA_DELAY + MORE_DELAY;");
    let output = checked(&source, config(60));
    assert!(output.contains("    output_signal <=\n        calculate_output(input_data, settings)\n            after PROPAGATION_DELAY;"), "{output}");
    for width in [36, 45, 60] {
        let output = checked(&source, config(width));
        assert!(
            !output.lines().any(|line| line.trim() == "after"),
            "{output}"
        );
        assert_width(&output, width);
    }
}

#[test]
fn expanded_untimed_waveforms_and_delay_mechanisms_keep_list_structure() {
    for delay in ["", "transport ", "reject 1 ns inertial "] {
        let source = architecture(&format!(
            "output_signal <= {delay}first_long_value, second_long_value, third_long_value;"
        ));
        let output = checked(&source, config(45));
        assert!(output.contains("output_signal <=\n"), "{output}");
        assert!(
            output.contains(
                "        first_long_value,\n        second_long_value,\n        third_long_value;"
            ),
            "{output}"
        );
        if !delay.is_empty() {
            assert!(
                output.contains(&format!("        {}\n", delay.trim())),
                "{output}"
            );
        }
        assert_width(&output, 45);
    }
}

#[test]
fn waveform_width_boundary_reserves_semicolon() {
    let source = architecture("x <= '1' after 10 ns;");
    let compact = checked(&source, config(25));
    assert!(compact.contains("    x <= '1' after 10 ns;"), "{compact}");
    let expanded = checked(&source, config(24));
    assert!(
        expanded.contains("    x <=\n        '1' after 10 ns;"),
        "{expanded}"
    );
}

#[test]
fn disabled_types_and_waveforms_remain_verbatim() {
    let disabled = "-- vhdl_ls off\n  type untouched is ( AA , BB , CC );\n-- vhdl_ls on\n";
    let source = format!(
        "package p is\n{disabled}type state_t is (FIRST_STATE, SECOND_STATE, THIRD_STATE); end;"
    );
    let output = checked(&source, config(40));
    assert!(output.contains(disabled), "{output}");
    assert!(output.contains("type state_t is (\n"), "{output}");
    let disabled = "-- vhdl_ls off\n  x<= a after 1 ns,b after 2 ns;\n-- vhdl_ls on\n";
    let output = checked(
        &architecture(&format!("\n{disabled}x <= '1' after 10 ns;")),
        config(40),
    );
    assert!(output.contains(disabled), "{output}");
    assert!(output.contains("x <= '1' after 10 ns;"), "{output}");
}

#[test]
fn selected_and_conditional_waveforms_keep_timing_and_modifiers() {
    let body = "with mode select output_signal <= transport first_long_value after first_long_delay when 0, unaffected when others; output_signal <= reject 1 ns inertial first_long_value after first_long_delay when enabled else second_long_value after second_long_delay;";
    for body in [
        body.to_string(),
        format!("process begin {body} wait; end process;"),
    ] {
        let output = checked(&architecture(&body), config(50));
        assert!(output.contains("transport"), "{output}");
        assert!(output.contains("reject 1 ns inertial"), "{output}");
        assert!(output.contains("unaffected when others"), "{output}");
        assert!(output.contains("after first_long_delay"), "{output}");
        assert!(
            !output.lines().any(|line| line.trim() == "after"),
            "{output}"
        );
    }
}

#[test]
fn comments_and_nested_layouts_are_stable_across_settings() {
    let source = "package p is\ntype state_t is ( -- open\nIDLE, -- comma\n-- next state\nRUNNING, DONE -- end\n);\ntype matrix_t is array ( -- dimensions\nnatural range <>, -- next\nnatural range <> -- close\n) of -- element\nstd_logic_vector(31 downto 0);\nend;\nentity e is end; architecture rtl of e is begin\noutput_signal <= -- waveform\ncalculate(data, mode, enabled) -- value\nafter -- timing\nbase_delay + extra_delay, -- element\nsecond_value after final_delay;\nprocess begin x <= first_value after delay_a, second_value after delay_b; wait; end process;\nend;";
    for max_width in [28, 40, 60, 100] {
        for indent_width in [0, 2, 4] {
            for inline_argument_limit in [0, 2, 5] {
                for keyword_case in [KeywordCase::Lower, KeywordCase::Upper] {
                    for align_declarations in [false, true] {
                        checked(
                            source,
                            FormatConfig {
                                max_width,
                                indent_width,
                                inline_argument_limit,
                                keyword_case,
                                align_declarations,
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
fn compatible_syntax_has_identical_layout_across_standards() {
    let source = include_str!("formatting/types_and_waveforms.input.vhd");
    let expected = include_str!("formatting/types_and_waveforms.expected.vhd");
    for standard in [
        VHDLStandard::VHDL1993,
        VHDLStandard::VHDL2008,
        VHDLStandard::VHDL2019,
    ] {
        assert_eq!(checked_standard(source, config(60), standard), expected);
    }
}

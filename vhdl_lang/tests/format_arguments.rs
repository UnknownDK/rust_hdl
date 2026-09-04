use std::path::Path;
use vhdl_lang::{format_text_with_config, FormatConfig, KeywordCase, VHDLParser, VHDLStandard};

fn checked(input: &str, config: FormatConfig) -> String {
    let parser = VHDLParser::new(VHDLStandard::VHDL2008);
    let path = Path::new("arguments.vhd");
    let output = format_text_with_config(&parser, path, input, &config).unwrap();
    assert_eq!(
        output,
        format_text_with_config(&parser, path, &output, &config).unwrap(),
        "{config:?}"
    );
    output
}

#[test]
fn shared_limit_controls_declarations_calls_maps_and_interfaces() {
    for inline_argument_limit in [0, 1, 2, 3, 10] {
        for count in 1..=3 {
            let names = &["a", "bb", "ccc"][..count];
            let parameters = names
                .iter()
                .map(|name| format!("{name}: natural"))
                .collect::<Vec<_>>()
                .join("; ");
            let associations = names
                .iter()
                .map(|name| format!("{name} => {name}"))
                .collect::<Vec<_>>()
                .join(", ");
            let ports = names
                .iter()
                .map(|name| format!("{name}: in bit"))
                .collect::<Vec<_>>()
                .join("; ");
            let args = names.join(", ");
            let source = format!("entity e is generic({parameters}); port({ports}); end; architecture rtl of e is function foo({parameters}) return bit; procedure proc({parameters}); begin child: entity work.e generic map({associations}) port map({associations}); process begin proc({args}); x := foo({args}); end process; end;");
            let config = FormatConfig {
                max_width: 200,
                inline_argument_limit,
                ..FormatConfig::default()
            };
            let output = checked(&source, config);
            let newline = if count > inline_argument_limit {
                "\n"
            } else {
                "a"
            };
            for prefix in [
                "generic (",
                "port (",
                "function foo(",
                "procedure proc(",
                "generic map (",
                "port map (",
                "proc(",
                "x := foo(",
            ] {
                assert!(
                    output.contains(&format!("{prefix}{newline}")),
                    "{config:?}, count={count}, {prefix}\n{output}"
                );
            }
        }
    }
}

#[test]
fn requested_call_and_assert_style_is_the_default() {
    let input = "entity e is end; architecture rtl of e is begin process begin file_open(foo, bar, read_mode); assert false report \"Unsupported bla bla\" severity failure; end process; end;";
    let output = checked(input, FormatConfig::default());
    assert!(output.contains("        file_open(\n            foo,\n            bar,\n            read_mode\n        );"), "{output}");
    assert!(output.contains("        assert false\n            report \"Unsupported bla bla\"\n            severity failure;"), "{output}");
}

#[test]
fn expanded_function_keeps_return_and_is_with_closing_parenthesis() {
    let input = "package body p is function foo(a: natural; bb: natural; ccc: natural) return std_logic_vector is variable result: std_logic_vector(1 downto 0); begin return result; end function; end;";
    let output = checked(
        input,
        FormatConfig {
            align_declarations: true,
            ..FormatConfig::default()
        },
    );
    assert!(output.contains("    function foo(\n        a   : natural;\n        bb  : natural;\n        ccc : natural\n    ) return std_logic_vector is\n        variable result"), "{output}");
    let one = "package body p is function foo(bar: natural) return std_logic_vector is variable result: bit; begin return result; end; end;";
    let output = checked(
        one,
        FormatConfig {
            inline_argument_limit: 0,
            align_declarations: true,
            ..FormatConfig::default()
        },
    );
    assert!(
        output.contains("function foo(\n        bar : natural\n    ) return std_logic_vector is"),
        "{output}"
    );
}

#[test]
fn width_and_comments_can_expand_lists_below_the_limit() {
    let input = "package p is function foo(first_argument: natural; second_argument: natural) return std_logic_vector; end; entity e is end; architecture rtl of e is begin u: entity work.e generic map(first_argument => first_argument, second_argument => second_argument); process begin file_open(foo, -- keep this comment\nbar); end process; end;";
    for inline_argument_limit in [0, 2, 100] {
        let output = checked(
            input,
            FormatConfig {
                max_width: 42,
                inline_argument_limit,
                ..FormatConfig::default()
            },
        );
        assert!(output.contains("function foo(\n"), "{output}");
        assert!(output.contains("generic map (\n"), "{output}");
        assert!(output.contains("file_open(\n"), "{output}");
        assert!(output.lines().all(|line| line.len() <= 42), "{output}");
    }
}

#[test]
fn nested_calls_and_array_indices_follow_the_same_rule() {
    let input = "entity e is end; architecture rtl of e is begin process begin x := f(a, g(b, c, d)); y := matrix(i, j); z := tensor(i, j, k); end process; end;";
    let output = checked(input, FormatConfig::default());
    assert!(output.contains("g(\n"), "{output}");
    assert!(output.contains("matrix(i, j)"), "{output}");
    assert!(output.contains("tensor(\n"), "{output}");
    for inline_argument_limit in [0, 1, 2, 3] {
        for max_width in [30, 50, 100] {
            for keyword_case in [KeywordCase::Lower, KeywordCase::Upper] {
                checked(
                    input,
                    FormatConfig {
                        inline_argument_limit,
                        max_width,
                        keyword_case,
                        ..FormatConfig::default()
                    },
                );
            }
        }
    }
}

#[test]
fn grouped_parameters_count_each_name_without_rewriting_declarations() {
    let output = checked(
        "package p is function foo(a, b, c: natural) return bit; end;",
        FormatConfig::default(),
    );
    assert!(
        output.contains("function foo(\n        a, b, c: natural\n    ) return bit;"),
        "{output}"
    );
}

#[test]
fn assertion_clauses_expand_independently_of_argument_settings() {
    let input = "entity e is end; architecture rtl of e is begin check_it: postponed assert ok report \"why\" severity failure; assert ok severity warning; assert ok; process begin local_check: assert ok report \"why\"; end process; end;";
    for inline_argument_limit in [0, 2, 100] {
        let output = checked(
            input,
            FormatConfig {
                inline_argument_limit,
                ..FormatConfig::default()
            },
        );
        assert!(
            output.contains(
                "check_it: postponed assert ok\n        report \"why\"\n        severity failure;"
            ),
            "{output}"
        );
        assert!(
            output.contains("assert ok\n        severity warning;"),
            "{output}"
        );
        assert!(output.contains("assert ok;"));
        assert!(
            output.contains("local_check: assert ok\n            report \"why\";"),
            "{output}"
        );
    }
}

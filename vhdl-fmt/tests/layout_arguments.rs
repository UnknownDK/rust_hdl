// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this file,
// You can obtain one at http://mozilla.org/MPL/2.0/.

//! Carried over from the `vhdl_lang` formatter's tests.

mod compat;
use compat::*;
use std::path::Path;

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
#[ignore = "layout not ported: a break point is missing at narrow widths"]
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

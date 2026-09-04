use std::path::Path;
use vhdl_lang::{format_text_with_config, FormatConfig, KeywordCase, VHDLParser, VHDLStandard};

fn aligned() -> FormatConfig {
    FormatConfig {
        align_declarations: true,
        align_associations: true,
        ..FormatConfig::default()
    }
}

#[test]
fn representative_rtl_style_fixture() {
    let input = include_str!("formatting/alignment_rtl.input.vhd");
    assert_eq!(
        checked(input, aligned()),
        include_str!("formatting/alignment_rtl.expected.vhd")
    );
    for max_width in [50, 80, 100, 120] {
        for keyword_case in [KeywordCase::Lower, KeywordCase::Upper] {
            checked(
                input,
                FormatConfig {
                    max_width,
                    keyword_case,
                    ..aligned()
                },
            );
        }
    }
}

fn checked(input: &str, config: FormatConfig) -> String {
    let parser = VHDLParser::new(VHDLStandard::VHDL2008);
    let path = Path::new("alignment.vhd");
    let once = format_text_with_config(&parser, path, input, &config).unwrap();
    let twice = format_text_with_config(&parser, path, &once, &config).unwrap();
    assert_eq!(once, twice, "not idempotent at {config:?}");
    once
}

#[test]
fn selective_alignment_matches_style_b() {
    let input = "entity demo is\nport(clk: in bit; reset_n: in bit; output_valid: out boolean);\nend entity;\narchitecture rtl of demo is\nsignal ready: boolean := false;\nsignal transfer_count: natural := 0;\nbegin\nchild: entity work.demo port map(clk => clk, reset_n => reset_n, output_valid => ready);\nend architecture;";
    let expected = "entity demo is\n    port (\n        clk          : in bit;\n        reset_n      : in bit;\n        output_valid : out boolean\n    );\nend entity;\n\narchitecture rtl of demo is\n    signal ready          : boolean := false;\n    signal transfer_count : natural := 0;\nbegin\n    child: entity work.demo\n        port map (\n            clk          => clk,\n            reset_n      => reset_n,\n            output_valid => ready\n        );\nend architecture;\n";
    assert_eq!(checked(input, aligned()), expected);
    let plain = checked(input, FormatConfig::default());
    assert!(plain.contains("signal ready: boolean := false;"));
    assert!(plain.contains("clk => clk,"));
}

#[test]
fn alignment_switches_are_independent() {
    let input = "entity e is end; architecture rtl of e is signal a: bit; signal longer: bit; begin u: entity work.e port map(a => a, longer => longer); end;";
    let declarations = checked(
        input,
        FormatConfig {
            align_declarations: true,
            ..FormatConfig::default()
        },
    );
    assert!(declarations.contains("signal a      : bit;"));
    assert!(declarations.contains("a => a,"));
    let associations = checked(
        input,
        FormatConfig {
            align_associations: true,
            ..FormatConfig::default()
        },
    );
    assert!(associations.contains("signal a: bit;"));
    assert!(associations.contains("a      => a,"));
}

#[test]
fn blank_lines_comments_and_other_declarations_are_boundaries() {
    let input = "package p is\nsignal a: bit;\nsignal longer: bit;\n\nsignal b: bit;\n-- new group\nsignal much_longer: bit;\nconstant c: bit := '0';\ntype state_t is (idle, busy);\nsignal d: bit;\nsignal ee: bit;\nend;";
    let output = checked(input, aligned());
    assert!(
        output.contains("signal a      : bit;\n    signal longer : bit;\n\n    signal b : bit;"),
        "{output}"
    );
    assert!(
        output.contains(
            "-- new group\n    signal much_longer : bit;\n    constant c         : bit := '0';"
        ),
        "{output}"
    );
    assert!(
        output.contains("signal d  : bit;\n    signal ee : bit;"),
        "{output}"
    );
}

#[test]
fn map_and_interface_blank_lines_are_preserved_with_alignment() {
    let input = "entity e is port(a: in bit; longer: in bit;\n\nb: in bit; cc: in bit); end; architecture rtl of e is begin u: entity work.e port map(a => a, longer => longer,\n\nb => b, cc => cc); end;";
    let output = checked(input, aligned());
    assert!(
        output.contains("longer : in bit;\n\n        b  : in bit;"),
        "{output}"
    );
    assert!(
        output.contains("longer => longer,\n\n            b  => b,"),
        "{output}"
    );
}

#[test]
fn width_limits_win_and_wrapped_rows_do_not_widen_neighbors() {
    let input = "package p is\nsignal a: bit;\nsignal longer: bit;\nsignal extremely_long_name: integer := calculate(first_operand, second_operand);\nsignal b: bit;\nsignal cc: bit;\nend;";
    for max_width in 35..=110 {
        for indent_width in [0, 2, 4] {
            for keyword_case in [KeywordCase::Lower, KeywordCase::Upper] {
                let output = checked(
                    input,
                    FormatConfig {
                        max_width,
                        indent_width,
                        keyword_case,
                        ..aligned()
                    },
                );
                for line in output.lines() {
                    assert!(line.chars().count() <= max_width, "{max_width}: {output}");
                    assert!(!line.ends_with(' '));
                }
            }
        }
    }
    let output = checked(
        input,
        FormatConfig {
            max_width: 45,
            ..aligned()
        },
    );
    assert!(output.contains("signal a      : bit;"), "{output}");
    assert!(output.contains("signal b  : bit;"), "{output}");
}

#[test]
fn padding_falls_back_when_individually_short_rows_cannot_align() {
    let input = "package p is\nsignal a: integer := 123456789;\nsignal lengthy_name: bit;\nend;";
    let output = checked(
        input,
        FormatConfig {
            max_width: 38,
            ..aligned()
        },
    );
    assert!(
        output.contains("signal a : integer := 123456789;"),
        "{output}"
    );
    assert!(output.lines().all(|line| line.len() <= 38));
}

#[test]
fn comments_and_disabled_text_remain_exact() {
    let input = "package P is\nsignal a: bit; -- trailing æ €\n-- vhdl_ls off\n  invalid : => \"  \r\n-- vhdl_ls on\nsignal longer: bit;\nsignal b /* before colon */ : bit;\nend;";
    let output = checked(input, aligned());
    assert!(output.contains("-- trailing æ €"));
    assert!(output.contains("  invalid : => \"  \r\n"));
}

#[test]
fn record_fields_and_generic_maps_have_independent_groups() {
    let input = "entity e is generic(size: natural := 8; enable: boolean := true); end; architecture rtl of e is type payload_t is record valid: boolean; data_word: bit_vector(0 to 7);\n\nx: bit; yy: bit; end record; begin u: entity work.e generic map(size => 8, enable => true) port map(a => a, longer_name => open); end;";
    let output = checked(input, aligned());
    assert!(output.contains("size   : natural := 8;"), "{output}");
    assert!(output.contains("valid     : boolean;"), "{output}");
    assert!(output.contains("x  : bit;"), "{output}");
    assert!(output.contains("size   => 8,"), "{output}");
    assert!(output.contains("a           => a,"), "{output}");
}

#[test]
fn mixed_maps_nested_calls_and_comments_are_stable_at_many_widths() {
    let input = "entity e is end; architecture rtl of e is begin u: entity work.e generic map(8, flag => true, verbose_flag => false) port map(a => compute(first_operand, second_operand), longer_name => nested(a, nested(b, c)),\n-- output group\nb => b, cc => open); end;";
    for max_width in 30..=120 {
        for indent_width in [0, 2, 4] {
            let output = checked(
                input,
                FormatConfig {
                    max_width,
                    indent_width,
                    ..aligned()
                },
            );
            assert!(
                output.lines().all(|line| line.len() <= max_width),
                "{max_width}: {output}"
            );
        }
    }
}

#[test]
fn comments_at_each_declaration_token_gap_do_not_move() {
    let tokens = [
        "package", "p", "is", "signal", "a", ":", "bit", ";", "signal", "longer", ":", "bit", ";",
        "end", ";",
    ];
    for gap in 1..tokens.len() {
        let source = format!(
            "{} -- anchored\n{}",
            tokens[..gap].join(" "),
            tokens[gap..].join(" ")
        );
        checked(&source, aligned());
    }
}

use std::path::Path;
use vhdl_lang::{format_text_with_config, FormatConfig, KeywordCase, VHDLParser, VHDLStandard};

fn config(max_width: usize) -> FormatConfig {
    FormatConfig {
        max_width,
        align_declarations: true,
        align_associations: true,
        ..FormatConfig::default()
    }
}

fn checked(source: &str, config: FormatConfig) -> String {
    checked_standard(source, config, VHDLStandard::VHDL2008)
}

fn checked_standard(source: &str, config: FormatConfig, standard: VHDLStandard) -> String {
    let parser = VHDLParser::new(standard);
    let path = Path::new("declaration_details.vhd");
    // The public API verifies tokens, comment anchors and parsing.
    let output = format_text_with_config(&parser, path, source, &config)
        .unwrap_or_else(|error| panic!("{error:?}\n{source}"));
    let twice = format_text_with_config(&parser, path, &output, &config).unwrap();
    assert_eq!(output, twice, "{standard:?} {config:?}\n{source}");
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
fn reviewed_declaration_details_fixture() {
    assert_eq!(
        checked(
            include_str!("formatting/declaration_details.input.vhd"),
            config(60)
        ),
        include_str!("formatting/declaration_details.expected.vhd")
    );
}

#[test]
fn short_attributes_and_aliases_remain_inline() {
    let source = "package p is attribute flag: boolean; attribute flag of x: signal is true; attribute flag of all: signal is false; attribute flag of others: signal is true; alias word: bit_vector(0 to 7) is data; alias f is original[bit return bit]; end;";
    let output = checked(source, config(100));
    for statement in [
        "attribute flag: boolean;",
        "attribute flag of x: signal is true;",
        "attribute flag of all: signal is false;",
        "attribute flag of others: signal is true;",
        "alias word: bit_vector(0 to 7) is data;",
        "alias f is original[bit return bit];",
    ] {
        assert!(output.contains(statement), "{output}");
    }
}

#[test]
fn long_attributes_break_at_clause_boundaries() {
    let source = "package p is attribute very_long_attribute_name: boolean; attribute mark_debug of very_long_internal_signal_name: signal is true; attribute flag of original[bit return bit]: function is first_condition and second_condition; end;";
    let output = checked(source, config(60));
    assert!(output.contains("attribute mark_debug of\n        very_long_internal_signal_name: signal is\n        true;"), "{output}");
    let narrow = checked(source, config(40));
    assert!(
        narrow.contains("attribute very_long_attribute_name:\n        boolean;"),
        "{narrow}"
    );
    assert!(
        narrow.contains("very_long_internal_signal_name:\n            signal is\n        true;"),
        "{narrow}"
    );
    assert_width(&narrow, 40);
}

#[test]
fn aliases_prefer_complete_types_and_preserve_signatures() {
    let source = "package p is alias received_data_word: std_logic_vector(31 downto 0) is very_long_internal_data_bus(31 downto 0); alias aliased_function is original_function[bit return bit]; end;";
    let output = checked(source, config(60));
    assert!(output.contains("alias received_data_word:\n        std_logic_vector(31 downto 0) is\n        very_long_internal_data_bus(31 downto 0);"), "{output}");
    assert!(
        output.contains("alias aliased_function is\n        original_function[bit return bit];"),
        "{output}"
    );
    let typed_alias = "package p is alias received_data_word: std_logic_vector(31 downto 0) is very_long_internal_data_bus(31 downto 0); end;";
    let narrow = checked(typed_alias, config(40));
    assert!(
        narrow.contains("std_logic_vector(31 downto 0) is"),
        "{narrow}"
    );
    assert_width(&narrow, 40);
    let narrower = checked(typed_alias, config(38));
    assert!(narrower.contains("std_logic_vector(\n"), "{narrower}");
    assert_width(&narrower, 38);
}

#[test]
fn external_names_preserve_absolute_relative_and_package_paths() {
    for path in [
        ".tb.dut.internal_data_bus",
        "^.dut.internal_data_bus",
        "@work.data_pkg.internal_data_bus",
    ] {
        let source = format!("package p is alias debug_data is << signal {path}: std_logic_vector(31 downto 0) >>; end;");
        let output = checked(&source, config(60));
        assert!(
            output.contains("alias debug_data is\n        <<\n"),
            "{output}"
        );
        assert!(output.contains(path), "{output}");
        assert!(
            output.contains("std_logic_vector(31 downto 0)\n        >>;"),
            "{output}"
        );
        assert_width(&output, 60);
    }
    let output = checked(
        "package p is alias x is << signal .tb.x: bit >>; end;",
        config(100),
    );
    assert!(
        output.contains("alias x is << signal .tb.x : bit >>;"),
        "{output}"
    );
}

#[test]
fn width_expanded_maps_align_with_two_arguments() {
    let source = architecture("u: entity work.child port map (data => input_data_bus, output_ready => output_ready_signal);");
    let output = checked(&source, config(60));
    assert!(output.contains("port map (\n            data         => input_data_bus,\n            output_ready => output_ready_signal\n        );"), "{output}");
    let plain = checked(
        &source,
        FormatConfig {
            align_associations: false,
            ..config(60)
        },
    );
    assert!(plain.contains("data => input_data_bus,"), "{plain}");
    assert_width(&output, 60);
}

#[test]
fn width_expanded_ports_generics_and_parameters_align() {
    for source in [
        "entity e is port(data: in std_logic_vector(31 downto 0); output_ready: out boolean); end;",
        "entity e is generic(data: std_logic_vector(31 downto 0); output_ready: boolean); end;",
        "package p is function f(data: std_logic_vector(31 downto 0); output_ready: boolean) return bit; end;",
    ] {
        let output = checked(source, config(60));
        assert!(output.contains("        data         :"), "{output}");
        assert!(output.contains("        output_ready :"), "{output}");
        assert_width(&output, 60);
    }
}

#[test]
fn inline_lists_are_not_padded_or_forced_to_expand() {
    let source = "entity e is port(a: in bit; longer: out bit); end; architecture rtl of e is begin u: entity work.child port map(a => a, longer => b); end;";
    let output = checked(source, config(100));
    assert!(
        output.contains("port (a : in bit; longer : out bit);"),
        "{output}"
    );
    assert!(
        output.contains("port map (a => a, longer => b);"),
        "{output}"
    );
    // Exact unpadded line width must still fit even though aligned padding would not.
    let source = architecture("u: entity work.child port map(a => a, longer => b);");
    let line = "        port map (a => a, longer => b);";
    let output = checked(&source, config(line.len()));
    assert!(output.contains(line), "{output}");
    let narrower = checked(&source, config(line.len() - 1));
    assert!(narrower.contains("a      => a,"), "{narrower}");
}

#[test]
fn comments_blank_lines_and_positional_items_separate_runs() {
    let source = architecture("u: entity work.child port map(a => x, longer => y,\n\nb => z, cc => w, -- trailing comma\nd => x, eeee => y, positional, f => x, gg => y);");
    let output = checked(
        &source,
        FormatConfig {
            inline_argument_limit: 20,
            ..config(50)
        },
    );
    assert!(output.contains("a      => x,\n            longer => y,\n\n            b => z,\n            cc => w, -- trailing comma"), "{output}");
    assert!(output.contains("d    => x,\n            eeee => y,\n            positional,\n            f  => x,\n            gg => y"), "{output}");
    let ports = "entity e is port(a: bit; longer: bit; -- separator\nb: bit; cc: bit); end;";
    let output = checked(
        ports,
        FormatConfig {
            inline_argument_limit: 20,
            ..config(50)
        },
    );
    assert!(
        output.contains(
            "a : bit;\n        longer : bit; -- separator\n        b  : bit;\n        cc : bit"
        ),
        "{output}"
    );
}

#[test]
fn alignment_falls_back_when_combined_columns_exceed_width() {
    let source = architecture(
        "u: entity work.child port map(a => moderately_long_value, much_longer_formal => x);",
    );
    let output = checked(&source, config(45));
    assert!(output.contains("a => moderately_long_value,"), "{output}");
    assert!(output.contains("much_longer_formal => x"), "{output}");
    assert_width(&output, 45);
}

#[test]
fn comment_expansion_aligns_without_affecting_neighboring_inline_maps() {
    let source = architecture("u: entity work.child generic map ( -- parameters\na => x, longer => y) port map(a => a, longer => b);");
    let output = checked(&source, config(100));
    assert!(output.contains("generic map ( -- parameters\n            a      => x,\n            longer => y\n        )"), "{output}");
    assert!(
        output.contains("port map (a => a, longer => b);"),
        "{output}"
    );
}

#[test]
fn wrapped_rows_do_not_widen_neighboring_maps() {
    let source = architecture("u: entity work.child port map(a => x, bb => y, extremely_long_formal => calculate(first_input, second_input, third_input), c => z, dddd => w);");
    let output = checked(
        &source,
        FormatConfig {
            inline_argument_limit: 20,
            ..config(50)
        },
    );
    assert!(
        output.contains("a  => x,\n            bb => y,"),
        "{output}"
    );
    assert!(
        output.contains("c    => z,\n            dddd => w"),
        "{output}"
    );
    assert_width(&output, 50);
}

#[test]
fn comment_anchors_and_nested_layouts_are_stable_across_settings() {
    let source = "package p is\nattribute flag: -- type\nboolean;\nattribute flag of -- entity\nx: signal is -- expression\nfirst_condition and second_condition;\nalias word: -- subtype\nstd_logic_vector(31 downto 0) is -- target\ndata(31 downto 0);\nalias external_data is << -- open\nsignal .tb.dut.data: -- external type\nstd_logic_vector(31 downto 0) -- close\n>>;\nfunction f(a: bit; longer: bit) return bit;\nend; entity e is port(a: bit; longer: bit;\n\nb: bit; cc: bit); end; architecture rtl of e is begin u: entity work.e generic map(a => calculate(x, y), longer => z) port map(a => x, -- boundary\nlonger => y, b => x, cc => y); end;";
    for max_width in [32, 45, 60, 100] {
        for indent_width in [0, 2, 4] {
            for inline_argument_limit in [0, 2, 10] {
                for keyword_case in [KeywordCase::Lower, KeywordCase::Upper] {
                    for (align_declarations, align_associations) in
                        [(false, false), (true, false), (false, true), (true, true)]
                    {
                        checked(
                            source,
                            FormatConfig {
                                max_width,
                                indent_width,
                                inline_argument_limit,
                                keyword_case,
                                align_declarations,
                                align_associations,
                            },
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn syntax_versions_and_disabled_regions_are_preserved() {
    let source = "package p is attribute flag: boolean; attribute flag of long_signal_name: signal is true; alias word: bit_vector(0 to 7) is long_data_signal; end;";
    for standard in [
        VHDLStandard::VHDL1993,
        VHDLStandard::VHDL2008,
        VHDLStandard::VHDL2019,
    ] {
        assert_eq!(
            checked_standard(source, config(40), standard),
            checked(source, config(40))
        );
    }
    let fixture = include_str!("formatting/declaration_details.input.vhd");
    assert_eq!(
        checked_standard(fixture, config(60), VHDLStandard::VHDL2019),
        checked(fixture, config(60))
    );
    let raw = "-- vhdl_ls off\n  alias raw_name is original ;\n-- vhdl_ls on\n";
    let source = format!("package p is\n{raw}attribute flag: boolean; end;");
    assert!(checked(&source, config(40)).contains(raw));
}

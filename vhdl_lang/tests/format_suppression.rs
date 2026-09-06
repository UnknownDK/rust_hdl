use std::path::Path;
use vhdl_lang::{
    format_text_with_config, FormatConfig, FormatError, Source, VHDLParser, VHDLStandard,
};

fn format(input: &str) -> String {
    format_text_with_config(
        &VHDLParser::new(VHDLStandard::default()),
        Path::new("suppression.vhd"),
        input,
        &FormatConfig::default(),
    )
    .unwrap()
}

fn preserved(input: &str, region: &str) -> String {
    let output = format(input);
    assert!(output.contains(region), "{output}");
    assert_eq!(format(&output), output);
    output
}

#[test]
fn off_on_preserves_layout_without_hiding_code_from_the_parser() {
    for ending in ["\n", "\r\n", "\r"] {
        let region =
            "  -- fmt: off\n signal   a:bit; -- café € 😀\n signal longer   : bit;\n  -- fmt: on\n"
                .replace('\n', ending);
        let input = format!("PACKAGE p IS{ending}{region}signal b:bit; END;{ending}");
        let output = preserved(&input, &region);
        assert!(output.contains("    signal b: bit;"));
        let parser = VHDLParser::new(VHDLStandard::default());
        let mut diagnostics = Vec::new();
        let parsed = parser.parse_design_source(
            &Source::inline(Path::new("test.vhd"), &output),
            &mut diagnostics,
        );
        assert!(diagnostics.is_empty());
        let vhdl_lang::ast::AnyDesignUnit::Primary(vhdl_lang::ast::AnyPrimaryUnit::Package(
            package,
        )) = &parsed.design_units[0].1
        else {
            panic!("expected package")
        };
        assert_eq!(
            package.decl.len(),
            3,
            "suppressed declarations must remain in the AST"
        );
    }
}

#[test]
fn suppressed_code_is_still_validated() {
    let input = "package p is\n-- fmt: off\nthis is not vhdl\n-- fmt: on\nend;";
    let result = format_text_with_config(
        &VHDLParser::new(VHDLStandard::default()),
        Path::new("bad.vhd"),
        input,
        &FormatConfig::default(),
    );
    assert!(matches!(result, Err(FormatError::InputDiagnostics(_))));
}

#[test]
fn skip_supports_leading_and_trailing_comments_and_complete_compound_statements() {
    for region in [
        "  -- fmt: skip\n  signal a,b :bit;\n",
        "  signal a,b :bit; -- fmt: skip\n",
        "  -- fmt: skip\n  function f(a,b:bit) return bit is begin return a; end;\n",
    ] {
        preserved(
            &format!("package p is\n{region}signal other:bit; end;"),
            region,
        );
    }
    for region in [
        "-- fmt: skip\np:process begin if true then null; else null; end if; wait; end process;\n",
        "p:process begin if true then null; else null; end if; wait; end process; -- fmt: skip\n",
    ] {
        preserved(
            &format!("entity e is end; architecture a of e is begin\n{region}end;"),
            region,
        );
    }
}

#[test]
fn directives_in_strings_prose_and_legacy_regions_are_not_interpreted() {
    let input = "package p is\nconstant s:string:=\"-- fmt: off\";\n-- mention fmt: off\nsignal x:bit;\n-- vhdl_ls off\n-- fmt: skip\ninvalid\n-- vhdl_ls on\nend;";
    let output = preserved(
        input,
        "-- vhdl_ls off\n-- fmt: skip\ninvalid\n-- vhdl_ls on\n",
    );
    assert!(output.contains("    signal x: bit;"));
}

#[test]
fn block_directives_unmatched_off_and_adjacent_regions_are_stable() {
    for region in [
        "/* fmt: off */\nsignal x:bit;\n/* fmt: on */\n",
        "-- fmt: off\nsignal x:bit;\n-- fmt: on\n-- fmt: off\nsignal y:bit;\n-- fmt: on\n",
    ] {
        preserved(&format!("package p is\n{region}end;"), region);
    }
    let region = "-- fmt: off\nsignal x:bit;\nend;";
    preserved(&format!("package p is\n{region}"), region);
}

#[test]
fn misplaced_skip_reports_an_error() {
    let input = "package p is signal x: -- fmt: skip\nbit; end;";
    let result = format_text_with_config(
        &VHDLParser::new(VHDLStandard::default()),
        Path::new("bad.vhd"),
        input,
        &FormatConfig::default(),
    );
    assert!(matches!(result, Err(FormatError::InvalidSuppression(_))));
}

#[test]
fn suppression_at_token_gaps_preserves_comments_spelling_and_idempotence() {
    let tokens = "package P is constant x : integer := f ( 1 , 2 ) + 3 ; signal y : bit ; end ;"
        .split_whitespace()
        .collect::<Vec<_>>();
    let parser = VHDLParser::new(VHDLStandard::default());
    for start in 0..tokens.len() {
        for end in start..tokens.len() {
            let region = format!(
                "-- fmt: off\n{}\n-- fmt: on\n",
                tokens[start..end].join("  ")
            );
            let input = format!(
                "{}\n{region}{}",
                tokens[..start].join(" "),
                tokens[end..].join(" ")
            );
            for width in [20, 100] {
                let config = FormatConfig {
                    max_width: width,
                    align_declarations: true,
                    ..FormatConfig::default()
                };
                let format = |text: &str| {
                    format_text_with_config(&parser, Path::new("gaps.vhd"), text, &config).unwrap()
                };
                let output = format(&input);
                assert!(output.contains(&region), "{input}\n{output}");
                assert_eq!(format(&output), output);
            }
        }
    }
}

#[test]
fn skip_handles_interfaces_context_clauses_and_design_units() {
    for region in [
        "-- fmt: skip\n a,b : in bit;\n",
        " a,b : in bit; -- fmt: skip\n",
    ] {
        preserved(
            &format!("entity e is port (\n{region}c: out bit); end;"),
            region,
        );
    }
    let region = "-- fmt: skip\nlibrary   ieee;\n";
    preserved(&format!("{region}entity e is end;"), region);
    let region = "-- fmt: skip\nENTITY  e IS END;\n";
    preserved(region, region);
}

#[test]
fn unmatched_off_preserves_trailing_blank_lines() {
    let region = "-- fmt: off\nsignal x:bit;\nend;\n\n\n";
    let output = preserved(&format!("package p is\n{region}"), region);
    assert!(output.ends_with(region));
}

#[test]
fn legacy_and_formatter_suppression_preserve_mixed_line_endings() {
    for ending in ["\n", "\r\n", "\r"] {
        let legacy = "-- vhdl_ls off\ninvalid € \"\n-- vhdl_ls on\n".replace('\n', ending);
        let region =
            format!("-- fmt: off{ending}signal a,b:bit;{ending}{legacy}-- fmt: on{ending}");
        preserved(
            &format!("package p is{ending}{region}end;{ending}"),
            &region,
        );
        preserved(
            &format!("package p is{ending}{legacy}end;{ending}"),
            &legacy,
        );
    }
}

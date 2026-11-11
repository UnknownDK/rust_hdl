// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this file,
// You can obtain one at http://mozilla.org/MPL/2.0/.

//! Reviewed layouts, carried over from the `vhdl_lang` formatter. Two
//! expectations changed: in `positional_aggregates` a call is no longer
//! expanded just for having more than two arguments, and in
//! `declaration_details` the space before a colon also applies to aliases and
//! attributes.

use vhdl_fmt::{format, FormatConfig, Settings};

fn check(name: &str, input: &str, expected: &str, config: FormatConfig) {
    let settings = Settings {
        format: config,
        ..Settings::default()
    };
    let output =
        format(input.as_bytes(), &settings).unwrap_or_else(|error| panic!("{name}: {error}"));
    let output = String::from_utf8(output).unwrap();
    assert_eq!(output, expected, "{name}");
    let again = format(output.as_bytes(), &settings).unwrap();
    assert_eq!(
        String::from_utf8(again).unwrap(),
        expected,
        "{name} is not stable"
    );
}

macro_rules! fixture {
    ($name:literal, $config:expr) => {
        check(
            $name,
            include_str!(concat!("fixtures/", $name, ".input.vhd")),
            include_str!(concat!("fixtures/", $name, ".expected.vhd")),
            $config,
        )
    };
}

fn width(max_width: usize) -> FormatConfig {
    FormatConfig {
        max_width,
        ..FormatConfig::default()
    }
}

#[test]
fn structure() {
    fixture!("design_units", FormatConfig::default());
    fixture!("nesting", FormatConfig::default());
    fixture!("declarations", FormatConfig::default());
    fixture!("generate", FormatConfig::default());
    fixture!("comments", FormatConfig::default());
    fixture!("layout", FormatConfig::default());
}

#[test]
fn alignment() {
    fixture!(
        "alignment_rtl",
        FormatConfig {
            align_declarations: true,
            space_before_colon: true,
            align_associations: true,
            align_assignments: true,
            ..FormatConfig::default()
        }
    );
}

#[test]
fn wrapping() {
    fixture!(
        "readability",
        FormatConfig {
            align_associations: true,
            ..width(60)
        }
    );
    fixture!(
        "layout_consistency",
        FormatConfig {
            align_declarations: true,
            space_before_colon: true,
            ..width(60)
        }
    );
    fixture!(
        "declaration_details",
        FormatConfig {
            align_declarations: true,
            space_before_colon: true,
            align_associations: true,
            ..width(60)
        }
    );
    fixture!(
        "positional_aggregates",
        FormatConfig {
            indent_width: 2,
            ..width(60)
        }
    );
    fixture!("types_and_waveforms", width(60));
}

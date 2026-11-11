// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this file,
// You can obtain one at http://mozilla.org/MPL/2.0/.

//! The entry point that the layout tests carried over from the `vhdl_lang`
//! formatter were written against.

#![allow(dead_code, unused_imports)]

use std::path::Path;
use vhdl_fmt::Settings;
pub use vhdl_fmt::{FormatConfig, FormatError, KeywordCase};
pub use vhdl_syntax::standard::VHDLStandard;

pub struct VHDLParser {
    standard: VHDLStandard,
}

impl VHDLParser {
    pub fn new(standard: VHDLStandard) -> Self {
        Self { standard }
    }
}

pub fn format_text_with_config(
    parser: &VHDLParser,
    _path: &Path,
    text: &str,
    config: &FormatConfig,
) -> Result<String, FormatError> {
    let settings = Settings {
        format: *config,
        standard: parser.standard,
    };
    vhdl_fmt::format(text.as_bytes(), &settings)
        .map(|output| String::from_utf8(output).expect("UTF-8 in, UTF-8 out"))
}

pub struct Source {
    text: String,
}

impl Source {
    pub fn inline(_path: &Path, text: &str) -> Self {
        Self {
            text: text.to_owned(),
        }
    }
}

pub fn format_source_with_config(
    parser: &VHDLParser,
    source: &Source,
    config: &FormatConfig,
) -> Result<String, FormatError> {
    format_text_with_config(parser, Path::new(""), &source.text, config)
}

impl VHDLParser {
    /// Collects the syntax errors of `source`.
    pub fn parse_design_source(&self, source: &Source, diagnostics: &mut Vec<String>) {
        let (_, errors) = vhdl_syntax::parser::parse_with_standard(self.standard, &source.text);
        diagnostics.extend(errors.iter().map(|error| error.to_string()));
    }
}

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this file,
// You can obtain one at http://mozilla.org/MPL/2.0/.

use super::VHDLFormatter;
use crate::ast::DesignFile;
use crate::{Diagnostic, Source, VHDLParser};
use std::error::Error;
use std::fmt;
use std::path::PathBuf;

/// An error encountered while formatting or validating a VHDL source.
#[derive(Debug)]
pub enum FormatError {
    InputDiagnostics(Vec<Diagnostic>),
    OutputDiagnostics(Vec<Diagnostic>),
    DesignUnitCountMismatch {
        input: usize,
        output: usize,
    },
    TokenCountMismatch {
        design_unit: usize,
        input: usize,
        output: usize,
    },
    TokenMismatch {
        design_unit: usize,
        token: usize,
    },
}

impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InputDiagnostics(diagnostics) => write!(
                f,
                "input contains {} parse diagnostic(s)",
                diagnostics.len()
            ),
            Self::OutputDiagnostics(diagnostics) => write!(
                f,
                "formatted output contains {} parse diagnostic(s)",
                diagnostics.len()
            ),
            Self::DesignUnitCountMismatch { input, output } => write!(
                f,
                "formatted output has {output} design unit(s), expected {input}"
            ),
            Self::TokenCountMismatch {
                design_unit,
                input,
                output,
            } => write!(
                f,
                "formatted design unit {design_unit} has {output} token(s), expected {input}"
            ),
            Self::TokenMismatch { design_unit, token } => write!(
                f,
                "formatted design unit {design_unit} differs at token {token}"
            ),
        }
    }
}

impl Error for FormatError {}

/// Format an in-memory VHDL source and verify that significant tokens are preserved.
pub fn format_source(parser: &VHDLParser, source: &Source) -> Result<String, FormatError> {
    let mut diagnostics = Vec::new();
    let input = parser.parse_design_source(source, &mut diagnostics);
    if !diagnostics.is_empty() {
        return Err(FormatError::InputDiagnostics(diagnostics));
    }

    let output = VHDLFormatter::format_design_file(&input);
    verify_output(parser, source, &input, &output)?;
    Ok(output)
}

fn verify_output(
    parser: &VHDLParser,
    source: &Source,
    input: &DesignFile,
    output: &str,
) -> Result<(), FormatError> {
    let mut output_path = PathBuf::from(source.file_name().as_os_str());
    output_path.as_mut_os_string().push(".rust_hdl_formatted");

    let mut diagnostics = Vec::new();
    let output_file =
        parser.parse_design_source(&Source::inline(&output_path, output), &mut diagnostics);
    if !diagnostics.is_empty() {
        return Err(FormatError::OutputDiagnostics(diagnostics));
    }

    if input.design_units.len() != output_file.design_units.len() {
        return Err(FormatError::DesignUnitCountMismatch {
            input: input.design_units.len(),
            output: output_file.design_units.len(),
        });
    }

    for (design_unit, ((input_tokens, _), (output_tokens, _))) in input
        .design_units
        .iter()
        .zip(&output_file.design_units)
        .enumerate()
    {
        if input_tokens.len() != output_tokens.len() {
            return Err(FormatError::TokenCountMismatch {
                design_unit,
                input: input_tokens.len(),
                output: output_tokens.len(),
            });
        }

        for (token, (input_token, output_token)) in
            input_tokens.iter().zip(output_tokens).enumerate()
        {
            if !input_token.equal_format(output_token) {
                return Err(FormatError::TokenMismatch { design_unit, token });
            }
        }
    }

    Ok(())
}

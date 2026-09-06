// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this file,
// You can obtain one at http://mozilla.org/MPL/2.0/.

use super::{FormatConfig, VHDLFormatter};
use crate::ast::DesignFile;
use crate::syntax::Value;
use crate::{Diagnostic, Source, VHDLParser};
use std::error::Error;
use std::fmt;
use std::path::PathBuf;

/// Format raw source text, preserving original disabled-region text (including
/// CRLF line endings) before `Source` performs its normal newline normalization.
pub fn format_text_with_config(
    parser: &VHDLParser,
    path: &std::path::Path,
    text: &str,
    config: &FormatConfig,
) -> Result<String, FormatError> {
    config.validate().map_err(FormatError::InvalidConfig)?;
    let disabled = super::disabled::DisabledRegions::new(text, parser.standard);
    let working_text = if disabled.is_empty() {
        text
    } else {
        &disabled.masked
    };
    let source = Source::inline(path, working_text);
    let normalized = source_text(&source);
    let mut diagnostics = Vec::new();
    let input = parser.parse_design_source(&source, &mut diagnostics);
    if !diagnostics.is_empty() {
        disabled.remap_diagnostics(&mut diagnostics, &Source::inline(path, text), text);
        return Err(FormatError::InputDiagnostics(diagnostics));
    }
    let output = VHDLFormatter::format_design_file_with_config(&input, config);
    let output_file = verify_output(parser, &source, &normalized, &input, &output)?;
    let output = if let Some(restored) =
        super::suppression::restore(working_text, &input, &output, &output_file)?
    {
        // Suppression changes only layout. Verify the complete result again,
        // including the code whose original spelling and whitespace we restored.
        let normalized_output = source_text(&Source::inline(path, &restored));
        verify_output(parser, &source, &normalized, &input, &normalized_output)?;
        restored
    } else {
        output
    };
    if disabled.is_empty() {
        Ok(output)
    } else {
        disabled
            .restore(&output)
            .ok_or(FormatError::DisabledRegionMismatch)
    }
}

/// An error encountered while formatting or validating a VHDL source.
#[derive(Debug)]
pub enum FormatError {
    InvalidConfig(String),
    InvalidSuppression(String),
    DisabledRegionMismatch,
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
    CommentMismatch {
        comment: usize,
    },
}

impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSuppression(message) => {
                write!(f, "invalid formatter directive: {message}")
            }
            Self::InvalidConfig(message) => write!(f, "invalid formatter configuration: {message}"),
            Self::DisabledRegionMismatch => {
                write!(f, "formatted output lost a disabled-region anchor")
            }
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
            Self::CommentMismatch { comment } => {
                write!(f, "formatted output differs at comment {comment}")
            }
        }
    }
}

impl Error for FormatError {}

/// Format an in-memory VHDL source and verify that significant tokens are preserved.
pub fn format_source(parser: &VHDLParser, source: &Source) -> Result<String, FormatError> {
    format_source_with_config(parser, source, &FormatConfig::default())
}

/// Format and verify a source using explicit style options.
pub fn format_source_with_config(
    parser: &VHDLParser,
    source: &Source,
    config: &FormatConfig,
) -> Result<String, FormatError> {
    format_text_with_config(parser, source.file_name(), &source_text(source), config)
}

fn source_text(source: &Source) -> String {
    let contents = source.contents();
    (0..contents.num_lines())
        .filter_map(|line| contents.get_line(line))
        .collect()
}

fn verify_output(
    parser: &VHDLParser,
    source: &Source,
    input_text: &str,
    input: &DesignFile,
    output: &str,
) -> Result<DesignFile, FormatError> {
    let mut output_path = PathBuf::from(source.file_name().as_os_str());
    output_path.as_mut_os_string().push(".rust_hdl_formatted");

    let mut diagnostics = Vec::new();
    let output_source = Source::inline(&output_path, output);
    let output_file = parser.parse_design_source(&output_source, &mut diagnostics);
    if !diagnostics.is_empty() {
        return Err(FormatError::OutputDiagnostics(diagnostics));
    }

    if input.design_units.len() != output_file.design_units.len() {
        return Err(FormatError::DesignUnitCountMismatch {
            input: input.design_units.len(),
            output: output_file.design_units.len(),
        });
    }

    let mut input_reader = SpellingCursor::new(input_text);
    let mut output_reader = SpellingCursor::new(output);
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
            if !input_token.equal_format(output_token)
                || !same_spelling(
                    &mut input_reader,
                    input_token,
                    &mut output_reader,
                    output_token,
                )
            {
                return Err(FormatError::TokenMismatch { design_unit, token });
            }
        }
    }

    let input_comments = comments(input);
    let output_comments = comments(&output_file);
    if input_comments.len() != output_comments.len() {
        return Err(FormatError::CommentMismatch {
            comment: input_comments.len().min(output_comments.len()),
        });
    }
    for (comment, ((input_gap, input_comment), (output_gap, output_comment))) in
        input_comments.iter().zip(output_comments).enumerate()
    {
        if *input_gap != output_gap
            || input_comment.value != output_comment.value
            || input_comment.multi_line != output_comment.multi_line
        {
            return Err(FormatError::CommentMismatch { comment });
        }
    }

    Ok(output_file)
}

/// Source slices preserve spelling without allocating per-token strings. ASCII
/// lines use direct offsets; UTF-16 positions on other lines use forward-only
/// cursors, so even a long non-ASCII line is scanned at most once.
pub(super) struct SpellingCursor<'a> {
    text: &'a str,
    lines: Vec<(usize, bool)>,
    line: usize,
    character: u32,
    offset: usize,
}

impl<'a> SpellingCursor<'a> {
    pub(super) fn new(text: &'a str) -> Self {
        let mut start = 0;
        let mut lines = Vec::new();
        let bytes = text.as_bytes();
        let mut index = 0;
        let mut ascii = true;
        while index < bytes.len() {
            ascii &= bytes[index].is_ascii();
            if matches!(bytes[index], b'\r' | b'\n') {
                lines.push((start, ascii));
                index += if bytes[index..].starts_with(b"\r\n") {
                    2
                } else {
                    1
                };
                start = index;
                ascii = true;
            } else {
                index += 1;
            }
        }
        if start < text.len() {
            lines.push((start, ascii));
        }
        lines.push((text.len(), true));
        Self {
            text,
            lines,
            line: 0,
            character: 0,
            offset: 0,
        }
    }

    pub(super) fn offset(&mut self, pos: crate::Position) -> usize {
        let line = pos.line as usize;
        let (start, ascii) = self.lines[line];
        if ascii {
            return start + pos.character as usize;
        }
        if self.line != line {
            self.line = line;
            self.character = 0;
            self.offset = start;
        }
        for ch in self.text[self.offset..].chars() {
            if self.character >= pos.character {
                break;
            }
            self.character += ch.len_utf16() as u32;
            self.offset += ch.len_utf8();
        }
        self.offset
    }

    fn token(&mut self, token: &crate::Token) -> &'a str {
        let start = self.offset(token.pos.start());
        let end = self.offset(token.pos.end());
        &self.text[start..end]
    }
}

fn same_spelling(
    left: &mut SpellingCursor<'_>,
    a: &crate::Token,
    right: &mut SpellingCursor<'_>,
    b: &crate::Token,
) -> bool {
    let left = left.token(a);
    let right = right.token(b);
    if matches!(a.value, Value::None) {
        left.eq_ignore_ascii_case(right)
    } else {
        left == right
    }
}

/// A comment must remain in the same gap between significant tokens. Leading
/// versus trailing attachment may change when wrapping, but crossing a token
/// (and therefore an expression or statement) is rejected.
fn comments(file: &DesignFile) -> Vec<(usize, &crate::syntax::Comment)> {
    let mut comments = Vec::new();
    let mut gap = 0;
    for (tokens, _) in &file.design_units {
        for token in tokens {
            if let Some(token_comments) = &token.comments {
                comments.extend(token_comments.leading.iter().map(|comment| (gap, comment)));
                comments.extend(
                    token_comments
                        .trailing
                        .iter()
                        .map(|comment| (gap + 1, comment)),
                );
            }
            gap += 1;
        }
    }
    comments.extend(file.final_comments.iter().map(|comment| (gap, comment)));
    comments
}

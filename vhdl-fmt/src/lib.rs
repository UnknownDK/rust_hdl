// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this file,
// You can obtain one at http://mozilla.org/MPL/2.0/.

//! A VHDL formatter built on the lossless syntax tree of `vhdl_syntax`.
//!
//! [`format`] works on bytes: VHDL source is Latin-1, but comments may hold any
//! encoding, and every byte outside the whitespace is written back unchanged.

pub mod config;
mod format;
// Groups, fill and alignment are unused until node-specific rules need them.
#[allow(dead_code)]
mod layout;
mod verify;

pub use config::{FormatConfig, KeywordCase, Settings, CONFIG_NAME};

use std::fmt;
use vhdl_syntax::parser;
use vhdl_syntax::tokens::TokenKind;

/// A one-based line and column (in bytes) of the input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Location {
    pub line: usize,
    pub column: usize,
}

impl Location {
    fn of(input: &[u8], offset: usize) -> Self {
        let before = &input[..offset.min(input.len())];
        let line_start = before
            .iter()
            .rposition(|&byte| byte == b'\n')
            .map_or(0, |index| index + 1);
        Self {
            line: before.iter().filter(|&&byte| byte == b'\n').count() + 1,
            column: before.len() - line_start + 1,
        }
    }
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.column)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxError {
    pub location: Location,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormatError {
    InvalidConfig(String),
    /// The input does not parse; it is never formatted.
    Syntax(Vec<SyntaxError>),
    /// The formatter would have changed more than layout. This is a bug in the
    /// formatter; no output is produced.
    Unsafe {
        location: Location,
        message: String,
    },
}

impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfig(message) => write!(f, "invalid configuration: {message}"),
            Self::Syntax(errors) => {
                for (index, error) in errors.iter().enumerate() {
                    if index > 0 {
                        writeln!(f)?;
                    }
                    write!(f, "{}: {}", error.location, error.message)?;
                }
                Ok(())
            }
            Self::Unsafe { location, message } => write!(
                f,
                "{location}: formatting would change the code ({message}); this is a formatter bug"
            ),
        }
    }
}

impl std::error::Error for FormatError {}

/// Format a VHDL source file.
///
/// The result is checked to hold the same tokens and comments as the input
/// before it is returned. Line endings follow the first one in the input.
pub fn format(input: &[u8], settings: &Settings) -> Result<Vec<u8>, FormatError> {
    settings
        .format
        .validate()
        .map_err(FormatError::InvalidConfig)?;
    let (file, errors) = parser::parse_with_standard(settings.standard, input);
    // A file holding only comments or whitespace is no design file to the
    // parser, but there is nothing in it that formatting could break.
    let only_trivia = file
        .visit_tokens()
        .all(|token| token.kind() == TokenKind::Eof);
    if !errors.is_empty() && !only_trivia {
        return Err(FormatError::Syntax(
            errors
                .iter()
                .map(|error| SyntaxError {
                    location: Location::of(input, error.span().start),
                    message: error.err().to_string(),
                })
                .collect(),
        ));
    }
    let mut emitter = format::Emitter::new(&settings.format, settings.standard);
    emitter.node(&file);
    let mut text = emitter.finish();
    let first_break = input.iter().position(|&byte| byte == b'\n');
    if first_break.is_some_and(|index| index > 0 && input[index - 1] == b'\r') {
        text = text.replace('\n', "\r\n");
    }
    let output = format::encode(&text);
    verify::verify(&file, &output, settings.standard, !only_trivia).map_err(|mismatch| {
        FormatError::Unsafe {
            location: Location::of(input, mismatch.offset),
            message: mismatch.message,
        }
    })?;
    Ok(output)
}

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this file,
// You can obtain one at http://mozilla.org/MPL/2.0/.

//! Proves that formatting changed nothing but layout: the output must parse
//! without errors into the same tokens, with the same comments before each.

use crate::format::{comment_text, decode, is_predefined_attribute};
use vhdl_syntax::parser;
use vhdl_syntax::standard::VHDLStandard;
use vhdl_syntax::syntax::{SyntaxNode, SyntaxToken};
use vhdl_syntax::tokens::TokenKind;

/// A difference between input and output, located in the input.
pub(crate) struct Mismatch {
    pub offset: usize,
    pub message: String,
}

fn comments(token: &SyntaxToken) -> Vec<String> {
    token
        .leading_trivia()
        .iter()
        .filter_map(comment_text)
        .collect()
}

fn describe(token: &SyntaxToken) -> String {
    if token.kind() == TokenKind::Eof {
        "the end of the file".to_owned()
    } else {
        format!("`{}`", decode(token.text().as_bytes()))
    }
}

pub(crate) fn verify(
    input: &SyntaxNode,
    output: &[u8],
    standard: VHDLStandard,
    must_parse: bool,
) -> Result<(), Mismatch> {
    let (formatted, errors) = parser::parse_with_standard(standard, output);
    let mut formatted = formatted.visit_tokens();
    let mut last = 0;
    let mut after_tick = false;
    for token in input.visit_tokens() {
        let offset = token.text_offset();
        last = offset;
        let mismatch = |message| Mismatch { offset, message };
        let Some(other) = formatted.next() else {
            return Err(mismatch(format!(
                "the output ends before {}",
                describe(&token)
            )));
        };
        // Keywords and predefined attribute names may change case.
        let cased = match token.kind() {
            TokenKind::Keyword(_) => true,
            TokenKind::Identifier => after_tick && is_predefined_attribute(token.text().as_bytes()),
            _ => false,
        };
        after_tick = token.kind() == TokenKind::Tick;
        let same_text = if cased {
            token.text().eq_ignore_case(other.text())
        } else {
            token.text() == other.text()
        };
        if token.kind() != other.kind() || !same_text {
            return Err(mismatch(format!(
                "{} became {}",
                describe(&token),
                describe(&other)
            )));
        }
        if comments(&token) != comments(&other) {
            return Err(mismatch(format!(
                "the comments before {} changed",
                describe(&token)
            )));
        }
    }
    if let Some(extra) = formatted.next() {
        return Err(Mismatch {
            offset: last,
            message: format!("the output has an extra {}", describe(&extra)),
        });
    }
    // Identical tokens parse identically, so this cannot fail on its own. It
    // is checked last so that a token difference is reported as the cause.
    if let Some(error) = errors.first().filter(|_| must_parse) {
        return Err(Mismatch {
            offset: last,
            message: format!("the output does not parse: {}", error.err()),
        });
    }
    Ok(())
}

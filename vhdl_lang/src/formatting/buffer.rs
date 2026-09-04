// This Source Code Form is subject to the terms of the Mozilla Public
// Lic// License, v. 2.0. If a copy of the MPL was not distributed with this file,
// This Source Code Form is subject to the terms of the Mozilla Public
// You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2024, Olof Kraigher olof.kraigher@gmail.com

use super::{
    layout::{self, Doc},
    FormatConfig, KeywordCase,
};
use crate::syntax::{Comment, Value};
use crate::{kind_str, Token};
use std::cell::OnceCell;

/// Builds a nested document from tokens, comments and structural whitespace.
/// Rendering is deferred until the completed document is requested.
pub struct Buffer {
    docs: Vec<Doc>,
    rendered: OnceCell<String>,
    has_content: bool,
    last_whitespace: bool,
    line_start: bool,
    group_starts: Vec<usize>,
    config: FormatConfig,
    /// Number of line breaks to emit before the next content.
    pending_line_breaks: usize,
    /// insert an extra newline before pushing a token.
    /// This is relevant when there is a trailing comment
    insert_extra_newline: bool,
    /// The current indentation level
    indentation: usize,
}

impl Buffer {
    pub fn new() -> Buffer {
        Self::with_config(FormatConfig::default())
    }

    pub fn with_config(config: FormatConfig) -> Buffer {
        Buffer {
            docs: Vec::new(),
            rendered: OnceCell::new(),
            has_content: false,
            last_whitespace: false,
            line_start: true,
            group_starts: Vec::new(),
            config,
            pending_line_breaks: 0,
            insert_extra_newline: false,
            indentation: 0,
        }
    }
}

impl Default for Buffer {
    fn default() -> Self {
        Self::new()
    }
}

/// Returns whether a leading comment is on the same line as the token, i.e.,
/// check the case
/// ```vhdl
/// /* some comment */ token
/// ```
fn leading_comment_is_on_token_line(comment: &Comment, token: &Token) -> bool {
    if !comment.multi_line {
        return false;
    }
    if comment.range.start.line != comment.range.end.line {
        return false;
    }
    token.pos.start().line == comment.range.start.line
}

impl From<Buffer> for String {
    fn from(mut value: Buffer) -> Self {
        value.flush_line_breaks();
        layout::render_docs(
            &value.docs,
            value.config.indent_width,
            value.config.max_width,
        )
    }
}

impl Buffer {
    pub fn as_str(&self) -> &str {
        self.rendered.get_or_init(|| {
            layout::render_docs(&self.docs, self.config.indent_width, self.config.max_width)
        })
    }

    /// pushes a whitespace character to the buffer
    pub fn push_whitespace(&mut self) {
        if !self.insert_extra_newline
            && self.pending_line_breaks == 0
            && self.has_content
            && !self.last_whitespace
        {
            self.push_doc(Doc::space());
            self.last_whitespace = true;
        }
    }

    fn format_comment(&mut self, comment: &Comment) {
        if !comment.multi_line {
            self.push_text(format!("--{}", comment.value));
        } else {
            self.push_text(format!("/*{}*/", comment.value));
        }
    }

    pub(crate) fn format_comments(&mut self, comments: &[Comment]) {
        for (i, comment) in comments.iter().enumerate() {
            self.format_comment(comment);
            if let Some(next_comment) = comments.get(i + 1) {
                let number_of_line_breaks =
                    (next_comment.range.start.line - comment.range.end.line).clamp(1, 2);
                self.line_breaks(number_of_line_breaks);
            } else {
                self.line_break();
            }
        }
    }

    fn flush_line_breaks(&mut self) {
        if self.pending_line_breaks == 0 {
            return;
        }
        if self.has_content {
            for _ in 0..self.pending_line_breaks {
                self.push_doc(Doc::hard_line());
            }
        }
        self.pending_line_breaks = 0;
        self.last_whitespace = true;
        self.line_start = true;
    }

    fn prepare_content(&mut self) {
        self.flush_line_breaks();
    }

    /// Push a token to this buffer.
    /// This takes care of all the leading and trailing comments attached to that token.
    pub fn push_token(&mut self, token: &Token) {
        if self.insert_extra_newline {
            self.line_break();
        }
        self.insert_extra_newline = false;
        let leading_start = self.docs.len();
        if let Some(comments) = &token.comments {
            // This is for example the case for situations like
            // some_token /* comment in between */ some_other token
            if comments.leading.len() == 1
                && leading_comment_is_on_token_line(&comments.leading[0], token)
            {
                self.format_comment(&comments.leading[0]);
                self.push_ch(' ');
            } else if !comments.leading.is_empty() {
                // A standalone leading comment stays before this token, not
                // after the previous token when an enclosing group flattens.
                if self.has_content && !self.line_start && self.pending_line_breaks == 0 {
                    self.line_break();
                }
                self.format_comments(comments.leading.as_slice());
                self.flush_line_breaks();
                // Leading comments belong before, not inside, a layout group
                // that starts at this token. They must not force the following
                // otherwise-short statement to break.
                for start in &mut self.group_starts {
                    if *start == leading_start {
                        *start = self.docs.len();
                    }
                }
            }
        }
        let spelling = match &token.value {
            Value::Identifier(ident) => ident.to_string(),
            Value::String(string) => format!("\"{}\"", string.to_string().replace('"', "\"\"")),
            Value::BitString(value, _) | Value::AbstractLiteral(value, _) => value.to_string(),
            Value::Character(ch) => format!("'{}'", *ch as char),
            Value::Text(text) => text.to_string(),
            Value::None => {
                // Token kinds, never identifier text, decide case conversion.
                let spelling = kind_str(token.kind);
                match self.config.keyword_case {
                    KeywordCase::Lower => spelling.to_owned(),
                    KeywordCase::Upper => spelling.to_ascii_uppercase(),
                }
            }
        };
        self.push_text(spelling);
        if let Some(comments) = &token.comments {
            if let Some(trailing_comment) = &comments.trailing {
                self.push_ch(' ');
                self.format_comment(trailing_comment);
                self.insert_extra_newline = true
            }
        }
    }

    fn push_str(&mut self, value: &str) {
        self.push_text(value.to_owned());
    }

    fn push_text(&mut self, value: String) {
        self.prepare_content();
        if !value.is_empty() {
            self.has_content = true;
            self.last_whitespace = value.ends_with(char::is_whitespace);
            self.line_start = value.ends_with('\n');
            self.push_doc(Doc::text_at(self.indentation, value));
        }
    }

    fn push_ch(&mut self, char: char) {
        self.push_str(&char.to_string());
    }

    /// Increase the indentation level.
    /// Indentation is emitted lazily at the start of rendered content lines,
    /// using the configured indentation width.
    ///
    /// This call should always be matched with a `decrease_indent` call.
    /// There is also the `indented` macro that combines the two calls.
    pub fn increase_indent(&mut self) {
        self.indentation += 1;
    }

    pub fn decrease_indent(&mut self) {
        self.indentation = self
            .indentation
            .checked_sub(1)
            .expect("formatter indentation underflow");
    }

    pub fn with_indent<R>(&mut self, format: impl FnOnce(&mut Self) -> R) -> R {
        self.increase_indent();
        let result = format(self);
        self.decrease_indent();
        result
    }

    fn push_doc(&mut self, doc: Doc) {
        self.rendered.take();
        self.docs.push(doc);
    }

    /// Compose a nested document without rendering any child buffer.
    pub(crate) fn group<R>(&mut self, build: impl FnOnce(&mut Self) -> R) -> R {
        self.group_with(build, false, false)
    }

    pub(crate) fn fill_group<R>(&mut self, build: impl FnOnce(&mut Self) -> R) -> R {
        self.group_with(build, true, false)
    }

    pub(crate) fn expression_group<R>(&mut self, build: impl FnOnce(&mut Self) -> R) -> R {
        self.group_with(build, false, true)
    }

    fn group_with<R>(&mut self, build: impl FnOnce(&mut Self) -> R, fill: bool, anchor: bool) -> R {
        self.prepare_content();
        self.group_starts.push(self.docs.len());
        let indent = self.indentation;
        let result = build(self);
        let start = self.group_starts.pop().expect("balanced document groups");
        let children = self.docs.split_off(start);
        let doc = Doc::concat(children);
        self.push_doc(Doc::indent(
            indent,
            if anchor {
                Doc::expression_group(doc)
            } else if fill {
                Doc::fill_group(doc)
            } else {
                Doc::group(doc)
            },
        ));
        result
    }

    pub(crate) fn soft_line(&mut self) {
        self.soft_break(true);
    }

    pub(crate) fn soft_break(&mut self, space: bool) {
        if self.insert_extra_newline || self.pending_line_breaks > 0 {
            self.line_break();
        } else {
            self.push_doc(Doc::soft_line(space));
            self.last_whitespace = true;
        }
    }

    /// Inserts a line break (i.e., newline) at the current position
    pub fn line_break(&mut self) {
        self.insert_extra_newline = false;
        self.pending_line_breaks = self.pending_line_breaks.max(1);
    }

    /// Ensures exactly one empty line before the next content.
    pub fn blank_line(&mut self) {
        self.insert_extra_newline = false;
        self.pending_line_breaks = 2;
    }

    /// Inserts multiple line breaks.
    /// Note that this method must always be used (i.e., is different from
    /// multiple `line_break` calls) as this method only indents the last line break
    pub fn line_breaks(&mut self, count: u32) {
        self.insert_extra_newline = false;
        match count {
            0 => {}
            1 => self.pending_line_breaks = self.pending_line_breaks.max(1),
            _ => self.pending_line_breaks = 2,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::analysis::tests::Code;
    use crate::formatting::buffer::Buffer;
    use std::iter::zip;

    fn check_token_formatted(input: &str, expected: &[&str]) {
        let code = Code::new(input);
        let tokens = code.tokenize();
        for (token, expected) in zip(tokens, expected) {
            let mut buffer = Buffer::new();
            buffer.push_token(&token);
            assert_eq!(buffer.as_str(), *expected);
        }
    }

    #[test]
    fn format_simple_token() {
        check_token_formatted("entity", &["entity"]);
        check_token_formatted("foobar", &["foobar"]);
        check_token_formatted("1 23 4E5 4e5", &["1", "23", "4E5", "4e5"]);
    }

    #[test]
    fn preserves_identifier_casing() {
        check_token_formatted("FooBar foobar", &["FooBar", "foobar"]);
    }

    #[test]
    fn character_formatting() {
        check_token_formatted("'a' 'Z' '''", &["'a'", "'Z'", "'''"]);
    }

    #[test]
    fn string_formatting() {
        check_token_formatted(
            r#""ABC" "" "DEF" """"  "Hello "" ""#,
            &["\"ABC\"", "\"\"", "\"DEF\"", "\"\"\"\"", "\"Hello \"\" \""],
        );
    }

    #[test]
    fn bit_string_formatting() {
        check_token_formatted(r#"B"10" 20B"8" X"2F""#, &["B\"10\"", "20B\"8\"", "X\"2F\""]);
    }

    #[test]
    fn leading_comment() {
        check_token_formatted(
            "\
-- I am a comment
foobar
        ",
            &["\
-- I am a comment
foobar"],
        );
    }

    #[test]
    fn multiple_leading_comments() {
        check_token_formatted(
            "\
-- I am a comment
-- So am I
foobar
        ",
            &["\
-- I am a comment
-- So am I
foobar"],
        );
    }

    #[test]
    fn trailing_comments() {
        check_token_formatted(
            "\
foobar --After foobar comes foobaz
        ",
            &["foobar --After foobar comes foobaz"],
        );
    }

    #[test]
    fn single_multiline_comment() {
        check_token_formatted(
            "\
/** Some documentation.
  * This is a token named 'entity'
  */
entity
        ",
            &["\
/** Some documentation.
  * This is a token named 'entity'
  */
entity"],
        );
    }

    #[test]
    fn multiline_comment_and_simple_comment() {
        check_token_formatted(
            "\
/* I am a multiline comment */
-- And I am a single line comment
entity
        ",
            &["\
/* I am a multiline comment */
-- And I am a single line comment
entity"],
        );
    }

    #[test]
    fn leading_comment_and_trailing_comment() {
        check_token_formatted(
            "\
-- Leading comment
entity -- Trailing comment
        ",
            &["\
-- Leading comment
entity -- Trailing comment"],
        );
    }

    #[test]
    fn structural_line_breaks_are_canonical_and_indent_lazily() {
        let mut buffer = Buffer::new();
        buffer.push_str("first");
        buffer.line_break();
        buffer.line_break();
        buffer.with_indent(|buffer| buffer.push_str("second"));
        buffer.blank_line();
        buffer.blank_line();
        buffer.push_str("third");
        buffer.line_break();

        assert_eq!(String::from(buffer), "first\n    second\n\nthird\n");
    }
}

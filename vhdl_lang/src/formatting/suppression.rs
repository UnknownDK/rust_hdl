// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this file,
// You can obtain one at http://mozilla.org/MPL/2.0/.

//! Restore formatting-only suppressions using verified token/comment positions.
//! Suppressed code is parsed normally and remains visible to language analysis.

use super::{api::SpellingCursor, FormatError};
use crate::ast::{
    search::{Search, Searcher},
    DesignFile,
};
use crate::{Position, Range, TokenAccess, TokenSpan};
use std::collections::BTreeMap;

struct Atom<'a> {
    range: Range,
    comment: Option<&'a str>,
    start: usize,
    end: usize,
}

fn atoms<'a>(file: &'a DesignFile, text: &str) -> Vec<Atom<'a>> {
    let mut result = Vec::new();
    for (tokens, _) in &file.design_units {
        for token in tokens {
            result.push(Atom {
                range: token.pos.range,
                comment: None,
                start: 0,
                end: 0,
            });
            if let Some(comments) = &token.comments {
                for comment in comments.leading.iter().chain(comments.trailing.iter()) {
                    result.push(Atom {
                        range: comment.range,
                        comment: Some(&comment.value),
                        start: 0,
                        end: 0,
                    });
                }
            }
        }
    }
    for comment in &file.final_comments {
        result.push(Atom {
            range: comment.range,
            comment: Some(&comment.value),
            start: 0,
            end: 0,
        });
    }
    result.sort_by_key(|atom| atom.range.start);
    let mut cursor = SpellingCursor::new(text);
    for atom in &mut result {
        atom.start = cursor.offset(atom.range.start);
        atom.end = cursor.offset(atom.range.end);
    }
    result
}

#[derive(Default)]
struct Statements {
    starts: BTreeMap<Position, Position>,
    ends: BTreeMap<Position, Position>,
}

impl Searcher for Statements {
    fn visit_statement_span(&mut self, ctx: &dyn TokenAccess, span: TokenSpan) {
        let pos = span.pos(ctx);
        self.starts
            .entry(pos.start())
            .and_modify(|end| *end = (*end).max(pos.end()))
            .or_insert(pos.end());
        self.ends
            .entry(pos.end())
            .and_modify(|start| *start = (*start).min(pos.start()))
            .or_insert(pos.start());
    }
}

fn line_start(text: &str, offset: usize) -> usize {
    text[..offset]
        .rfind(['\r', '\n'])
        .map_or(0, |index| index + 1)
}

fn expand_start(text: &str, offset: usize) -> usize {
    let start = line_start(text, offset);
    if text[start..offset].trim().is_empty() {
        start
    } else {
        offset
    }
}

fn expand_end(text: &str, offset: usize) -> usize {
    let end = text[offset..]
        .find(['\r', '\n'])
        .map_or(text.len(), |index| {
            let index = offset + index;
            index
                + if text[index..].starts_with("\r\n") {
                    2
                } else {
                    1
                }
        });
    if text[offset..end].trim().is_empty() {
        end
    } else {
        offset
    }
}

pub(super) fn restore(
    input: &str,
    input_file: &DesignFile,
    output: &str,
    output_file: &DesignFile,
) -> Result<Option<String>, FormatError> {
    if !input.contains("fmt:") {
        return Ok(None);
    }
    let original = atoms(input_file, input);
    let formatted = atoms(output_file, output);
    let mut statements = Statements::default();
    for (tokens, unit) in &input_file.design_units {
        let _ = unit.search(tokens, &mut statements);
    }
    let mut ranges = Vec::new();
    let mut off = None;
    for (index, atom) in original.iter().enumerate() {
        match atom.comment.map(str::trim) {
            Some("fmt: off") if off.is_none() => off = Some(index),
            Some("fmt: on") => {
                if let Some(start) = off.take() {
                    ranges.push((start, index));
                }
            }
            Some("fmt: skip") if off.is_none() => {
                let standalone = input[line_start(input, atom.start)..atom.start]
                    .trim()
                    .is_empty();
                let span = if standalone {
                    original[index + 1..]
                        .iter()
                        .position(|next| next.comment.is_none())
                        .and_then(|next| {
                            let start = index + 1 + next;
                            let end_pos = statements.starts.get(&original[start].range.start)?;
                            let end = original
                                .partition_point(|item| item.range.end <= *end_pos)
                                .checked_sub(1)?;
                            Some((index, end))
                        })
                } else {
                    original[..index]
                        .iter()
                        .rposition(|previous| previous.comment.is_none())
                        .and_then(|end| {
                            let start_pos = statements.ends.get(&original[end].range.end)?;
                            let start =
                                original.partition_point(|item| item.range.start < *start_pos);
                            Some((start, index))
                        })
                };
                ranges.push(span.ok_or_else(|| FormatError::InvalidSuppression(format!(
                    "fmt: skip on line {} must precede a complete declaration/statement or follow its final token",
                    atom.range.start.line + 1,
                )))?);
            }
            _ => {}
        }
    }
    if let Some(start) = off {
        ranges.push((start, original.len() - 1));
    }
    if ranges.is_empty() {
        return Ok(None);
    }
    ranges.sort_unstable();
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for (start, end) in ranges {
        if let Some(last) = merged.last_mut().filter(|last| start <= last.1) {
            last.1 = last.1.max(end);
        } else {
            merged.push((start, end));
        }
    }
    let mut result = String::new();
    let mut copied = 0;
    for (start, end) in merged {
        let input_start = expand_start(input, original[start].start);
        let through_eof =
            off.is_some_and(|unmatched| start <= unmatched && end == original.len() - 1);
        let input_end = if through_eof {
            input.len()
        } else {
            expand_end(input, original[end].end)
        };
        let output_start = expand_start(output, formatted[start].start);
        let output_end = if through_eof {
            output.len()
        } else {
            expand_end(output, formatted[end].end)
        };
        result.push_str(&output[copied..output_start]);
        result.push_str(&input[input_start..input_end]);
        copied = output_end;
    }
    result.push_str(&output[copied..]);
    Ok(Some(result))
}

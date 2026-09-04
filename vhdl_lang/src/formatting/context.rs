// This Source Code Form is subject to the terms of the Mozilla Public
// Lic// License, v. 2.0. If a copy of the MPL was not distributed with this file,
// This Source Code Form is subject to the terms of the Mozilla Public
// You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2024, Olof Kraigher olof.kraigher@gmail.com

use crate::ast::{ContextClause, ContextDeclaration, ContextItem};
use crate::formatting::buffer::Buffer;
use crate::formatting::VHDLFormatter;
use crate::syntax::Value;
use crate::{HasTokenSpan, TokenAccess, TokenSpan};
use vhdl_lang::indented;

impl VHDLFormatter<'_> {
    pub fn format_context(&self, context: &ContextDeclaration, buffer: &mut Buffer) {
        // context <name> is
        self.format_token_span(
            TokenSpan::new(context.span.start_token, context.span.start_token + 2),
            buffer,
        );
        indented!(buffer, {
            if !context.items.is_empty() {
                buffer.line_break();
            }
            self.format_context_clause(&context.items, buffer);
        });
        buffer.line_break();
        self.format_token_span(
            TokenSpan::new(context.end_token, context.span.end_token - 1),
            buffer,
        );
        self.format_token_id(context.span.end_token, buffer);
    }

    pub fn format_context_clause(&self, clause: &ContextClause, buffer: &mut Buffer) {
        self.format_context_clause_with_grouping(clause, false, buffer);
    }

    fn format_context_clause_with_grouping(
        &self,
        clause: &ContextClause,
        grouped: bool,
        buffer: &mut Buffer,
    ) {
        for (i, item) in clause.iter().enumerate() {
            match item {
                ContextItem::Use(use_clause) => self.format_use_clause(use_clause, buffer),
                ContextItem::Library(library_clause) => {
                    self.format_library_clause(library_clause, buffer)
                }
                ContextItem::Context(context_reference) => {
                    self.format_context_reference(context_reference, buffer)
                }
            }
            if let Some(next) = clause.get(i + 1) {
                let blank = grouped
                    && (matches!(item, ContextItem::Library(_))
                        && !matches!(next, ContextItem::Library(_))
                        || matches!((item, next), (ContextItem::Use(_), ContextItem::Use(_)))
                            && self.context_use_root(item) != self.context_use_root(next));
                if blank {
                    buffer.blank_line();
                } else {
                    self.line_break_preserve_whitespace(item.span().end_token, buffer);
                }
            }
        }
    }

    fn context_use_root(&self, item: &ContextItem) -> Option<String> {
        let ContextItem::Use(use_clause) = item else {
            return None;
        };
        let mut names = use_clause.name_list.iter();
        let root = match &self.tokens.index(names.next()?.span.start_token).value {
            Value::Identifier(symbol) => symbol.name_utf8(),
            _ => return None,
        };
        for name in names {
            match &self.tokens.index(name.span.start_token).value {
                Value::Identifier(symbol) if symbol.name_utf8() == root => {}
                _ => return None,
            }
        }
        Some(root)
    }

    /// Context items remain a group, separated from the following design unit.
    /// Context declarations use `format_context_clause` directly for their body.
    pub(crate) fn format_design_context(&self, clause: &ContextClause, buffer: &mut Buffer) {
        self.format_context_clause_with_grouping(clause, true, buffer);
        if !clause.is_empty() {
            buffer.blank_line();
        }
    }
}

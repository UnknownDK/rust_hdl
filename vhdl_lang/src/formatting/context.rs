// This Source Code Form is subject to the terms of the Mozilla Public
// Lic// License, v. 2.0. If a copy of the MPL was not distributed with this file,
// This Source Code Form is subject to the terms of the Mozilla Public
// You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2024, Olof Kraigher olof.kraigher@gmail.com

use crate::ast::{ContextClause, ContextDeclaration, ContextItem};
use crate::formatting::buffer::Buffer;
use crate::formatting::VHDLFormatter;
use crate::TokenSpan;
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
        let mut has_library = false;
        for (i, item) in clause.iter().enumerate() {
            if let ContextItem::Library(library) = item {
                self.format_library_clause(library, buffer);
                has_library = true;
            } else {
                let format_item = |buffer: &mut Buffer| match item {
                    ContextItem::Use(clause) => self.format_use_clause(clause, buffer),
                    ContextItem::Context(reference) => {
                        self.format_context_reference(reference, buffer)
                    }
                    ContextItem::Library(_) => unreachable!(),
                };
                if has_library {
                    buffer.with_indent(format_item);
                } else {
                    format_item(buffer);
                }
            }
            if let Some(next) = clause.get(i + 1) {
                if matches!(next, ContextItem::Library(_)) {
                    buffer.blank_line();
                } else {
                    buffer.line_break();
                }
            }
        }
    }

    /// Context items remain a group, separated from the following design unit.
    /// Context declarations use `format_context_clause` directly for their body.
    pub(crate) fn format_design_context(&self, clause: &ContextClause, buffer: &mut Buffer) {
        self.format_context_clause(clause, buffer);
        if !clause.is_empty() {
            buffer.blank_line();
        }
    }
}

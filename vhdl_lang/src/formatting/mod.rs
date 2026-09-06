// This Source Code Form is subject to the terms of the Mozilla Public
// Lic// License, v. 2.0. If a copy of the MPL was not distributed with this file,
// This Source Code Form is subject to the terms of the Mozilla Public
// You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2024, Olof Kraigher olof.kraigher@gmail.com

use crate::ast::DesignFile;
use crate::formatting::buffer::Buffer;
use crate::syntax::Kind;
use crate::{Token, TokenAccess};
use vhdl_lang::ast::HasIdent;

mod alignment;
mod api;
mod architecture;
mod buffer;
mod concurrent_statement;
mod configuration;
mod constraint;
mod context;
mod declaration;
mod design;
mod disabled;
mod entity;
mod expression;
mod interface;
mod layout;
mod name;
mod options;
mod sequential_statement;
mod statement;
mod subprogram;
mod suppression;
mod token;

pub use api::{format_source, format_source_with_config, format_text_with_config, FormatError};
pub use options::{FormatConfig, KeywordCase};

/// The formatter is the main entry point used for formatting a single
/// Design Unit from AST representation to string representation. In that sense,
/// the Formatter is the inverse to the Parser.
///
/// Most methods herein are called `format_<node>` where `node` is the AST node to format.
/// Methods compose documents in a mutable [Buffer]; the complete document is
/// rendered once with the configured width, indentation and keyword casing.
///
/// The formatter is capable of retaining comment information as well as preserving newlines.
pub struct VHDLFormatter<'b> {
    tokens: &'b Vec<Token>,
}

impl<'b> VHDLFormatter<'b> {
    pub fn new(tokens: &'b Vec<Token>) -> VHDLFormatter<'b> {
        VHDLFormatter { tokens }
    }

    /// Format an already-parsed design file using defaults. Prefer [format_source]
    /// when source preservation (including disabled regions) and verification
    /// are required; the AST omits disabled source text.
    pub fn format_design_file(file: &DesignFile) -> String {
        Self::format_design_file_with_config(file, &FormatConfig::default())
    }

    pub fn format_design_file_with_config(file: &DesignFile, config: &FormatConfig) -> String {
        let mut result = Buffer::with_config(*config);
        for (i, (tokens, design_unit)) in file.design_units.iter().enumerate() {
            let formatter = VHDLFormatter::new(tokens);
            formatter.format_any_design_unit(
                design_unit,
                &mut result,
                i == file.design_units.len() - 1,
            );
        }
        result.line_break();
        result.format_comments(&file.final_comments);
        result.line_break();
        result.into()
    }
}

impl VHDLFormatter<'_> {
    pub fn format_ident_list<T: HasIdent>(&self, idents: &[T], buffer: &mut Buffer) {
        self.format_ident_list_with_indent(idents, buffer, true);
    }

    pub(crate) fn format_declaration_idents<T: HasIdent>(&self, idents: &[T], buffer: &mut Buffer) {
        self.format_ident_list_with_indent(idents, buffer, false);
    }

    fn format_ident_list_with_indent<T: HasIdent>(
        &self,
        idents: &[T],
        buffer: &mut Buffer,
        continuation: bool,
    ) {
        buffer.expression_group(|buffer| {
            for (index, ident) in idents.iter().enumerate() {
                let token = ident.ident().token;
                if index == 0 {
                    self.format_token_id(token, buffer);
                } else if continuation {
                    buffer.with_indent(|buffer| {
                        buffer.soft_line();
                        self.format_token_id(token, buffer);
                    });
                } else {
                    buffer.soft_line();
                    self.format_token_id(token, buffer);
                }
                if self
                    .tokens
                    .get_token(token + 1)
                    .is_some_and(|token| token.kind == Kind::Comma)
                {
                    self.format_token_id(token + 1, buffer);
                }
            }
        });
    }
}

/// indents the provided block and de-indents at the end.
#[macro_export]
macro_rules! indented {
    ($buffer:ident, $block:block) => {
        $buffer.with_indent(|$buffer| $block);
    };
}

#[cfg(test)]
pub mod test_utils {
    use crate::formatting::buffer::Buffer;
    use crate::formatting::VHDLFormatter;
    use crate::syntax::test::Code;
    use vhdl_lang::VHDLStandard;

    pub(crate) fn check_formatted<T>(
        input: &str,
        expected: &str,
        to_ast: impl FnOnce(&Code) -> T,
        format: impl FnOnce(&VHDLFormatter<'_>, &T, &mut Buffer),
    ) {
        check_formatted_std(input, expected, VHDLStandard::default(), to_ast, format)
    }

    pub(crate) fn check_formatted_std<T>(
        input: &str,
        expected: &str,
        std: VHDLStandard,
        to_ast: impl FnOnce(&Code) -> T,
        format: impl FnOnce(&VHDLFormatter<'_>, &T, &mut Buffer),
    ) {
        let code = Code::with_standard(input, std);
        let ast_element = to_ast(&code);
        let tokens = code.tokenize();
        let formatter = VHDLFormatter::new(&tokens);
        let mut buffer = Buffer::new();
        format(&formatter, &ast_element, &mut buffer);
        assert_eq!(buffer.as_str(), expected);
    }
}

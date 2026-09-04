// This Source Code Form is subject to the terms of the Mozilla Public
// Lic// License, v. 2.0. If a copy of the MPL was not distributed with this file,
// This Source Code Form is subject to the terms of the Mozilla Public
// You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2024, Olof Kraigher olof.kraigher@gmail.com

use crate::ast::{
    ActualPart, AssociationElement, ElementMode, InterfaceDeclaration, InterfaceList,
    InterfaceObjectDeclaration, InterfacePackageDeclaration, InterfacePackageGenericMapAspect,
    InterfaceSubprogramDeclaration, MapAspect, ModeIndication, ModeViewElement,
    ModeViewIndicationKind, SeparatedList, SimpleModeIndication, SubprogramDefault,
};
use crate::formatting::buffer::Buffer;
use crate::formatting::VHDLFormatter;
use crate::syntax::Kind;
use crate::{HasTokenSpan, TokenAccess};
use vhdl_lang::ast::token_range::WithTokenSpan;
use vhdl_lang::ast::{InterfaceFileDeclaration, ModeViewIndication};
use vhdl_lang::TokenSpan;

impl VHDLFormatter<'_> {
    pub(crate) fn format_interface_list(&self, clause: &InterfaceList, buffer: &mut Buffer) {
        // A grouped declaration (`a, b, c: natural`) still declares three
        // arguments. Preserve its token structure while counting each name.
        let argument_count: usize = clause
            .items
            .iter()
            .map(|item| match item {
                InterfaceDeclaration::Object(object) => object.idents.len(),
                InterfaceDeclaration::File(file) => file.idents.len(),
                _ => 1,
            })
            .sum();
        let multiline = argument_count > buffer.config().inline_argument_limit;
        let span = clause.span;
        let end_token = if self.tokens.index(span.start_token).kind == Kind::LeftPar {
            span.start_token
        } else {
            span.start_token + 1
        };
        buffer.group(|buffer| {
            self.format_token_span(TokenSpan::new(span.start_token, end_token), buffer);
            if !clause.items.is_empty() {
                buffer.with_indent(|buffer| {
                    if !multiline {
                        buffer.soft_break(false);
                    } else {
                        buffer.line_break();
                    }
                    // Column alignment applies to structural multiline rows,
                    // never padding multiple items sharing a physical line.
                    let align = multiline && buffer.config().align_declarations;
                    self.format_aligned_items(
                        &clause.items,
                        buffer,
                        |item| {
                            if align {
                                match item {
                                    InterfaceDeclaration::Object(object) => {
                                        Some(object.colon_token)
                                    }
                                    InterfaceDeclaration::File(file) => Some(file.colon_token),
                                    _ => None,
                                }
                            } else {
                                None
                            }
                        },
                        |item| item.span(),
                        |i, item, buffer| {
                            self.format_interface_declaration(item, buffer);
                            if i < clause.items.len() - 1 {
                                self.format_token_id(item.get_end_token() + 1, buffer);
                                if align {
                                    self.line_break_preserve_whitespace(
                                        item.get_end_token() + 1,
                                        buffer,
                                    );
                                } else if multiline {
                                    buffer.line_break();
                                } else {
                                    buffer.soft_line();
                                }
                            }
                        },
                    );
                });
                if !multiline {
                    buffer.soft_break(false);
                } else {
                    buffer.line_break();
                }
            }
            if self.tokens.index(span.end_token).kind == Kind::SemiColon {
                self.format_token_id(span.end_token - 1, buffer);
            }
            self.format_token_id(span.end_token, buffer);
        });
    }

    pub fn format_map_aspect_span(
        &self,
        list: &SeparatedList<AssociationElement>,
        span: TokenSpan,
        buffer: &mut Buffer,
    ) {
        let multiline = list.items.len() > buffer.config().inline_argument_limit;
        buffer.group(|buffer| {
            self.format_token_span(
                TokenSpan::new(span.start_token, span.start_token + 2),
                buffer,
            );
            if !list.items.is_empty() {
                buffer.with_indent(|buffer| {
                    if multiline {
                        buffer.line_break();
                    } else {
                        buffer.soft_break(false);
                    }
                    let align = multiline && buffer.config().align_associations;
                    self.format_aligned_items(
                        &list.items,
                        buffer,
                        |item| {
                            if align {
                                item.formal.as_ref().map(|formal| formal.span.end_token + 1)
                            } else {
                                None
                            }
                        },
                        |item| {
                            TokenSpan::new(
                                item.formal
                                    .as_ref()
                                    .map_or(item.actual.span.start_token, |formal| {
                                        formal.span.start_token
                                    }),
                                item.actual.span.end_token,
                            )
                        },
                        |i, item, buffer| {
                            self.format_association_element(item, buffer);
                            if let Some(token) = list.tokens.get(i) {
                                self.format_token_id(*token, buffer);
                            }
                            if i + 1 < list.items.len() {
                                if align {
                                    self.line_break_preserve_whitespace(list.tokens[i], buffer);
                                } else if multiline {
                                    buffer.line_break();
                                } else {
                                    buffer.soft_line();
                                }
                            }
                        },
                    );
                });
                if multiline {
                    buffer.line_break();
                } else {
                    buffer.soft_break(false);
                }
            }
            self.format_token_id(span.end_token, buffer);
        });
    }

    pub fn format_map_aspect(&self, aspect: &MapAspect, buffer: &mut Buffer) {
        self.format_map_aspect_span(&aspect.list, aspect.span, buffer);
    }

    pub fn format_association_element(&self, element: &AssociationElement, buffer: &mut Buffer) {
        buffer.fill_group(|buffer| {
            if let Some(formal) = &element.formal {
                self.format_name(formal.as_ref(), buffer);
                buffer.push_whitespace();
                self.format_token_id(formal.span.end_token + 1, buffer);
                buffer.with_indent(|buffer| {
                    buffer.soft_line();
                    self.format_actual_part(&element.actual, buffer);
                });
            } else {
                self.format_actual_part(&element.actual, buffer);
            }
        });
    }

    pub fn format_actual_part(&self, actual_part: &WithTokenSpan<ActualPart>, buffer: &mut Buffer) {
        match &actual_part.item {
            ActualPart::Expression(expression) => self.format_conditional_expression(
                WithTokenSpan::new(expression, actual_part.span),
                buffer,
            ),
            ActualPart::Open => self.format_token_span(actual_part.span, buffer),
        }
    }

    pub fn format_interface_declaration(
        &self,
        declaration: &InterfaceDeclaration,
        buffer: &mut Buffer,
    ) {
        use InterfaceDeclaration::*;
        match declaration {
            Object(object) => self.format_interface_object(object, buffer),
            File(file) => self.format_interface_file_declaration(file, buffer),
            Type(type_decl) => {
                self.format_token_id(type_decl.tree.token - 1, buffer);
                buffer.push_whitespace();
                self.format_ident(type_decl, buffer);
            }
            Subprogram(subprogram) => {
                self.format_interface_subprogram_declaration(subprogram, buffer)
            }
            Package(package) => self.format_interface_package_declaration(package, buffer),
        }
    }

    pub fn format_interface_file_declaration(
        &self,
        declaration: &InterfaceFileDeclaration,
        buffer: &mut Buffer,
    ) {
        buffer.fill_group(|buffer| {
            self.format_token_id(declaration.span.start_token, buffer);
            buffer.push_whitespace();
            self.format_ident_list(&declaration.idents, buffer);
            if buffer.config().align_declarations {
                buffer.push_whitespace();
            }
            self.format_token_id(declaration.colon_token, buffer);
            buffer.with_indent(|buffer| {
                buffer.preferred_line();
                self.format_subtype_indication(&declaration.subtype_indication, buffer);
            });
        });
    }

    pub fn format_interface_subprogram_declaration(
        &self,
        subprogram: &InterfaceSubprogramDeclaration,
        buffer: &mut Buffer,
    ) {
        self.format_subprogram_specification(&subprogram.specification, buffer);
        if let Some(default) = &subprogram.default {
            buffer.push_whitespace();
            // is
            self.format_token_id(subprogram.specification.span().end_token + 1, buffer);
            buffer.push_whitespace();
            match default {
                SubprogramDefault::Name(name) => self.format_name(name.as_ref(), buffer),
                SubprogramDefault::Box => {
                    self.format_token_id(subprogram.specification.span().end_token + 2, buffer)
                }
            }
        }
    }

    pub fn format_interface_package_declaration(
        &self,
        package: &InterfacePackageDeclaration,
        buffer: &mut Buffer,
    ) {
        // package <ident> is new
        self.format_token_span(
            TokenSpan::new(package.span.start_token, package.span.start_token + 3),
            buffer,
        );
        buffer.push_whitespace();
        self.format_name(package.package_name.as_ref(), buffer);
        buffer.push_whitespace();
        let span = package.generic_map.span();
        match &package.generic_map.item {
            InterfacePackageGenericMapAspect::Map(map) => {
                self.format_map_aspect_span(map, span, buffer)
            }
            InterfacePackageGenericMapAspect::Box | InterfacePackageGenericMapAspect::Default => {
                // generic
                self.format_token_id(span.start_token, buffer);
                buffer.push_whitespace();
                // map
                self.format_token_id(span.start_token + 1, buffer);
                buffer.push_whitespace();
                //(
                self.format_token_id(span.start_token + 2, buffer);
                // <> | default
                self.format_token_id(span.start_token + 3, buffer);
                // )
                self.format_token_id(span.start_token + 4, buffer);
            }
        }
    }

    pub fn format_interface_object(
        &self,
        object: &InterfaceObjectDeclaration,
        buffer: &mut Buffer,
    ) {
        buffer.fill_group(|buffer| {
            // [signal] my_signal :
            let first_ident = object.idents[0].tree.token;
            if object.span.start_token != first_ident {
                self.format_token_span(
                    TokenSpan::new(object.span.start_token, first_ident - 1),
                    buffer,
                );
                buffer.push_whitespace();
                self.format_ident_list(&object.idents, buffer);
            } else {
                self.format_declaration_idents(&object.idents, buffer);
            }
            if buffer.config().align_declarations {
                buffer.push_whitespace();
            }
            self.format_token_id(object.colon_token, buffer);
            buffer.with_indent(|buffer| {
                buffer.preferred_line();
                self.format_mode(&object.mode, buffer);
            });
        });
    }

    pub fn format_mode(&self, mode: &ModeIndication, buffer: &mut Buffer) {
        use ModeIndication::*;
        match mode {
            Simple(simple) => self.format_simple_mode(simple, buffer),
            View(mode) => buffer.group(|buffer| self.format_mode_view_indication(mode, buffer)),
        }
    }

    pub fn format_mode_view_indication(&self, mode: &ModeViewIndication, buffer: &mut Buffer) {
        // view
        self.format_token_id(mode.span.start_token, buffer);
        buffer.push_whitespace();
        match &mode.kind {
            ModeViewIndicationKind::Array => {
                self.format_token_id(mode.name.span.start_token - 1, buffer);
                self.format_name(mode.name.as_ref(), buffer);
                self.format_token_id(mode.name.span.end_token + 1, buffer);
            }
            ModeViewIndicationKind::Record => {
                self.format_name(mode.name.as_ref(), buffer);
            }
        }
        if let Some((token, subtype)) = &mode.subtype_indication {
            buffer.push_whitespace();
            self.format_token_id(*token, buffer);
            buffer.push_whitespace();
            self.format_subtype_indication(subtype, buffer);
        }
    }

    pub fn format_element_mode(&self, mode: &ElementMode, buffer: &mut Buffer) {
        match mode {
            ElementMode::Simple(simple) => self.format_token_id(simple.token, buffer),
            ElementMode::Record(name) => {
                // view
                self.format_token_id(name.get_start_token() - 1, buffer);
                buffer.push_whitespace();
                self.format_name(name.as_ref(), buffer)
            }
            ElementMode::Array(name) => {
                //view (
                self.format_token_span(
                    TokenSpan::new(name.get_start_token() - 2, name.get_start_token() - 1),
                    buffer,
                );
                self.format_name(name.as_ref(), buffer);
                // )
                self.format_token_id(name.get_end_token() + 1, buffer);
            }
        }
    }

    pub fn format_mode_view_element(&self, mode: &ModeViewElement, buffer: &mut Buffer) {
        self.format_ident_list(&mode.names, buffer);
        self.format_token_id(mode.colon_token, buffer);
        buffer.push_whitespace();
        self.format_element_mode(&mode.mode, buffer);
        // ;
        self.format_token_id(mode.span.end_token, buffer);
    }

    pub fn format_simple_mode(&self, mode: &SimpleModeIndication, buffer: &mut Buffer) {
        buffer.group(|buffer| {
            if let Some(mode) = &mode.mode {
                self.format_token_id(mode.token, buffer);
                buffer.push_whitespace();
            }
            self.format_subtype_indication(&mode.subtype_indication, buffer);
        });
        self.format_default_expression(mode.expression.as_ref(), buffer);
    }
}

#[cfg(test)]
mod tests {
    use crate::analysis::tests::Code;
    use crate::formatting::test_utils::{check_formatted, check_formatted_std};
    use crate::VHDLStandard::VHDL2019;

    fn check_generic(input: &str) {
        check_formatted(
            input,
            input,
            Code::generic,
            |formatter, interface, buffer| {
                formatter.format_interface_declaration(interface, buffer)
            },
        );
    }

    #[test]
    fn format_simple_object() {
        check_generic("my_generic: natural");
    }

    #[test]
    fn format_simple_object_with_default() {
        check_generic("my_generic: natural := 7");
    }

    #[test]
    fn format_simple_object_with_explicit_mode() {
        check_generic("my_generic: in natural");
    }

    #[test]
    fn format_object_with_class() {
        check_generic("constant my_generic: in natural");
        check_generic("constant my_generic: natural");
    }

    fn check_element_mode(input: &str) {
        check_formatted_std(
            input,
            input,
            VHDL2019,
            Code::element_mode,
            |formatter, mode, buffer| formatter.format_element_mode(mode, buffer),
        );
    }

    #[test]
    fn format_element_mode() {
        check_element_mode("in");
        check_element_mode("out");
        check_element_mode("inout");
        check_element_mode("buffer");
        check_element_mode("linkage");
        check_element_mode("view foo");
        check_element_mode("view (foo)");
    }

    fn check_port(input: &str) {
        check_formatted_std(
            input,
            input,
            VHDL2019,
            Code::port,
            |formatter, interface, buffer| {
                formatter.format_interface_declaration(interface, buffer)
            },
        );
    }

    #[test]
    fn format_mode_view_indication() {
        check_port("signal foo: view bar");
        check_port("signal foo: view (bar)");
        check_port("signal foo: view bar of baz");
        check_port("signal foo: view (bar) of baz");
    }

    #[test]
    fn format_interface_file_declaration() {
        check_port("file valid: text");
    }

    #[test]
    fn format_interface_subprogram_declaration() {
        check_generic("function foo return bar");
        check_generic("procedure foo");
        check_generic("impure function foo return bar");
        check_generic("function foo return bar is lib.name");
        check_generic("function foo return bar is <>");
    }

    #[test]
    fn format_interface_package_declaration() {
        check_generic("package foo is new lib.pkg generic map (foo => bar)");
        check_generic("package foo is new lib.pkg generic map (<>)");
        check_generic("package foo is new lib.pkg generic map (default)");
    }
}

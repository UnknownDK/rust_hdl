// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this file,
// You can obtain one at http://mozilla.org/MPL/2.0/.

#[derive(Debug)]
pub(crate) enum Doc {
    Text(String),
    HardLine,
    SoftLine,
    Indent(Box<Doc>),
    Group(Box<Doc>),
    Concat(Vec<Doc>),
}

impl Doc {
    pub(crate) fn text(value: impl Into<String>) -> Self {
        Self::Text(value.into())
    }

    pub(crate) fn indent(doc: Doc) -> Self {
        Self::Indent(Box::new(doc))
    }

    pub(crate) fn group(doc: Doc) -> Self {
        Self::Group(Box::new(doc))
    }

    pub(crate) fn concat(docs: impl IntoIterator<Item = Doc>) -> Self {
        Self::Concat(docs.into_iter().collect())
    }
}

#[derive(Clone, Copy)]
enum Mode {
    Flat,
    Break,
}

pub(crate) fn render(
    doc: &Doc,
    output: &mut String,
    base_indent: usize,
    indent_width: usize,
    max_width: usize,
) {
    let column = current_column(output);
    let mut renderer = Renderer {
        output,
        base_indent,
        indent_width,
        max_width,
        column,
    };
    renderer.render(doc, 0, Mode::Break);
}

struct Renderer<'a> {
    output: &'a mut String,
    base_indent: usize,
    indent_width: usize,
    max_width: usize,
    column: usize,
}

impl Renderer<'_> {
    fn render(&mut self, doc: &Doc, indent: usize, mode: Mode) {
        match doc {
            Doc::Text(text) => self.write_text(text, indent),
            Doc::HardLine => self.newline(indent),
            Doc::SoftLine => match mode {
                Mode::Flat => {
                    self.output.push(' ');
                    self.column += 1;
                }
                Mode::Break => self.newline(indent),
            },
            Doc::Indent(doc) => self.render(doc, indent + 1, mode),
            Doc::Group(doc) => {
                let mode =
                    if flat_width(doc).is_some_and(|width| self.column + width <= self.max_width) {
                        Mode::Flat
                    } else {
                        Mode::Break
                    };
                self.render(doc, indent, mode);
            }
            Doc::Concat(docs) => {
                for doc in docs {
                    self.render(doc, indent, mode);
                }
            }
        }
    }

    fn newline(&mut self, indent: usize) {
        self.output.push('\n');
        let spaces = (self.base_indent + indent) * self.indent_width;
        self.output.extend(std::iter::repeat_n(' ', spaces));
        self.column = spaces;
    }

    fn write_text(&mut self, text: &str, indent: usize) {
        let mut lines = text.split_inclusive('\n').peekable();
        while let Some(line) = lines.next() {
            self.output.push_str(line);
            if line.ends_with('\n') {
                self.column = 0;
                if lines.peek().is_some() {
                    let spaces = (self.base_indent + indent) * self.indent_width;
                    self.output.extend(std::iter::repeat_n(' ', spaces));
                    self.column = spaces;
                }
            } else {
                self.column += line.chars().count();
            }
        }
    }
}

fn flat_width(doc: &Doc) -> Option<usize> {
    match doc {
        Doc::Text(text) if text.contains('\n') => None,
        Doc::Text(text) => Some(text.chars().count()),
        Doc::HardLine => None,
        Doc::SoftLine => Some(1),
        Doc::Indent(doc) | Doc::Group(doc) => flat_width(doc),
        Doc::Concat(docs) => docs
            .iter()
            .try_fold(0usize, |width, doc| Some(width + flat_width(doc)?)),
    }
}

fn current_column(output: &str) -> usize {
    output
        .rsplit_once('\n')
        .map_or(output.chars().count(), |(_, line)| line.chars().count())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grouped_list() -> Doc {
        Doc::group(Doc::concat([
            Doc::text("port ("),
            Doc::indent(Doc::concat([Doc::SoftLine, Doc::text("a: bit")])),
            Doc::SoftLine,
            Doc::text(");"),
        ]))
    }

    #[test]
    fn group_uses_spaces_when_it_fits() {
        let mut output = String::new();
        render(&grouped_list(), &mut output, 0, 4, 100);
        assert_eq!(output, "port ( a: bit );");
    }

    #[test]
    fn group_breaks_and_indents_when_it_does_not_fit() {
        let mut output = "    ".to_string();
        render(&grouped_list(), &mut output, 1, 4, 12);
        assert_eq!(output, "    port (\n        a: bit\n    );");
    }

    #[test]
    fn hard_lines_force_a_group_to_break() {
        let doc = Doc::group(Doc::concat([
            Doc::text("("),
            Doc::indent(Doc::concat([Doc::HardLine, Doc::text("a")])),
            Doc::HardLine,
            Doc::text(")"),
        ]));
        let mut output = String::new();
        render(&doc, &mut output, 0, 4, 100);
        assert_eq!(output, "(\n    a\n)");
    }
}

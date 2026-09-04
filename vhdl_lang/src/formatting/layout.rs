// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this file,
// You can obtain one at http://mozilla.org/MPL/2.0/.

/// Width summaries are computed once, when documents are composed. Measuring a
/// nested group never walks that group's children again.
#[derive(Debug, Clone, Copy)]
struct Prefix(usize);

impl Prefix {
    // One machine word per summary: the low bit marks a line break. A Rust
    // allocation cannot exceed isize::MAX bytes, so the remaining bits suffice
    // for any materializable line. Saturation also makes composition overflow-safe.
    fn new(width: usize, ends_line: bool) -> Self {
        Self((width.min(usize::MAX >> 1) << 1) | usize::from(ends_line))
    }

    fn width(self) -> usize {
        self.0 >> 1
    }
    fn ends_line(self) -> bool {
        self.0 & 1 != 0
    }
}

#[derive(Debug)]
pub(crate) struct Doc {
    kind: Kind,
    flat: Prefix,
    broken: Prefix,
}

#[derive(Debug)]
enum Kind {
    Text(String, Option<usize>),
    HardLine,
    Space,
    /// Lazy inter-token padding, resolved while composing an alignment run.
    Align(usize),
    SoftLine(bool),
    Indent(usize, Box<Doc>),
    Group {
        children: Vec<Doc>,
        indent: Option<usize>,
        fill: bool,
        anchor: bool,
    },
    Concat(Vec<Doc>),
}

impl Doc {
    fn leaf(kind: Kind, flat: Prefix, broken: Prefix) -> Self {
        Self { kind, flat, broken }
    }

    pub(crate) fn text(value: impl Into<String>) -> Self {
        let value = value.into();
        let prefix = Prefix::new(
            value.split('\n').next().unwrap_or("").chars().count(),
            value.contains('\n'),
        );
        Self::leaf(Kind::Text(value, None), prefix, prefix)
    }

    pub(crate) fn text_at(level: usize, value: String) -> Self {
        let mut doc = Self::text(value);
        if let Kind::Text(_, indent) = &mut doc.kind {
            *indent = Some(level);
        }
        doc
    }

    pub(crate) fn hard_line() -> Self {
        let prefix = Prefix::new(0, true);
        Self::leaf(Kind::HardLine, prefix, prefix)
    }

    pub(crate) fn space() -> Self {
        let prefix = Prefix::new(1, false);
        Self::leaf(Kind::Space, prefix, prefix)
    }

    pub(crate) fn align() -> Self {
        let prefix = Prefix::new(1, false);
        Self::leaf(Kind::Align(1), prefix, prefix)
    }

    pub(crate) fn is_space(&self) -> bool {
        matches!(self.kind, Kind::Space)
    }

    /// Only single-line rows participate. Widths are measured from documents,
    /// not source spacing or pre-rendered child buffers.
    pub(crate) fn alignment_widths(&self) -> Option<(usize, usize)> {
        if self.flat.ends_line() {
            return None;
        }
        self.alignment_column()
            .map(|left| (left, self.flat.width() - left))
    }

    fn alignment_column(&self) -> Option<usize> {
        match &self.kind {
            Kind::Align(_) => Some(0),
            Kind::Indent(_, child) => child.alignment_column(),
            Kind::Group { children, .. } | Kind::Concat(children) => {
                let mut width = 0usize;
                for child in children {
                    if let Some(column) = child.alignment_column() {
                        return Some(width + column);
                    }
                    width += child.flat.width();
                }
                None
            }
            _ => None,
        }
    }

    pub(crate) fn pad_alignment(&mut self, extra: usize) -> bool {
        let changed = match &mut self.kind {
            Kind::Align(width) => {
                *width += extra;
                true
            }
            Kind::Indent(_, child) => child.pad_alignment(extra),
            Kind::Group { children, .. } | Kind::Concat(children) => {
                children.iter_mut().any(|child| child.pad_alignment(extra))
            }
            _ => false,
        };
        if changed {
            match &self.kind {
                Kind::Align(width) => {
                    self.flat = Prefix::new(*width, false);
                    self.broken = self.flat;
                }
                Kind::Indent(_, child) => {
                    self.flat = child.flat;
                    self.broken = child.broken;
                }
                Kind::Group { children, .. } | Kind::Concat(children) => {
                    self.flat = Self::summarize(children, true);
                    self.broken = Self::summarize(children, false);
                }
                _ => unreachable!(),
            }
        }
        changed
    }

    /// A line break that flattens to a space (`space = true`) or nothing.
    pub(crate) fn soft_line(space: bool) -> Self {
        Self::leaf(
            Kind::SoftLine(space),
            Prefix::new(usize::from(space), false),
            Prefix::new(0, true),
        )
    }

    /// Absolute indentation level, applied lazily at the start of a line.
    pub(crate) fn indent(level: usize, mut doc: Doc) -> Self {
        match &mut doc.kind {
            Kind::Text(_, indent) | Kind::Group { indent, .. } => {
                *indent = Some(level);
                return doc;
            }
            _ => {}
        }
        let flat = doc.flat;
        let broken = doc.broken;
        Self {
            kind: Kind::Indent(level, Box::new(doc)),
            flat,
            broken,
        }
    }

    pub(crate) fn group(doc: Doc) -> Self {
        Self::group_with(doc, false, false)
    }

    /// Keep a statement prefix on the line if its children can wrap. Unlike a
    /// delimiter group, this need not expand just because a nested call expands.
    pub(crate) fn fill_group(doc: Doc) -> Self {
        Self::group_with(doc, true, false)
    }

    pub(crate) fn expression_group(doc: Doc) -> Self {
        Self::group_with(doc, false, true)
    }

    fn group_with(doc: Doc, fill: bool, anchor: bool) -> Self {
        let flat = doc.flat;
        let broken = doc.broken;
        let children = match doc.kind {
            Kind::Concat(children) => children,
            _ => vec![doc],
        };
        Self {
            kind: Kind::Group {
                children,
                indent: None,
                fill,
                anchor,
            },
            flat,
            broken,
        }
    }

    pub(crate) fn concat(docs: impl IntoIterator<Item = Doc>) -> Self {
        let docs: Vec<_> = docs.into_iter().collect();
        let flat = Self::summarize(&docs, true);
        let broken = Self::summarize(&docs, false);
        Self {
            kind: Kind::Concat(docs),
            flat,
            broken,
        }
    }

    fn summarize(docs: &[Doc], flat: bool) -> Prefix {
        let mut prefix = Prefix::new(0usize, false);
        for doc in docs {
            let next = if flat { doc.flat } else { doc.broken };
            prefix = Prefix::new(
                prefix.width().saturating_add(next.width()),
                next.ends_line(),
            );
            if next.ends_line() {
                break;
            }
        }
        prefix
    }
}

#[derive(Clone, Copy)]
enum Mode {
    Flat,
    Break,
    Fill,
}

#[cfg(test)]
fn render(doc: &Doc, indent_width: usize, max_width: usize) -> String {
    render_docs(std::slice::from_ref(doc), indent_width, max_width)
}

pub(crate) fn render_docs(docs: &[Doc], indent_width: usize, max_width: usize) -> String {
    let mut output = String::new();
    let mut column = 0usize;
    let mut line_indent = 0usize;
    let mut space = 0usize;
    let mut stack: Vec<_> = docs
        .iter()
        .rev()
        .map(|doc| (doc, 0usize, Mode::Break, 0isize))
        .collect();
    while let Some((doc, indent, mode, adjustment)) = stack.pop() {
        match &doc.kind {
            Kind::Text(text, level) => {
                let indent = level.map_or(indent, |level| level.saturating_add_signed(adjustment));
                if text.is_empty() {
                    continue;
                }
                if column == 0 {
                    line_indent = indent;
                    let padding = indent.saturating_mul(indent_width);
                    output.extend(std::iter::repeat_n(' ', padding));
                    column = padding;
                } else if space > 0 {
                    output.extend(std::iter::repeat_n(' ', space));
                    column += space;
                }
                space = 0;
                output.push_str(text);
                column = if let Some((_, last)) = text.rsplit_once('\n') {
                    last.chars().count()
                } else {
                    column + text.chars().count()
                };
            }
            Kind::Space => space = space.max(usize::from(column != 0)),
            Kind::Align(width) => space = space.max(if column == 0 { 0 } else { *width }),
            Kind::SoftLine(true) if matches!(mode, Mode::Flat) => {
                space = space.max(usize::from(column != 0))
            }
            Kind::SoftLine(false) if matches!(mode, Mode::Flat) => {}
            Kind::SoftLine(flat_space) if matches!(mode, Mode::Fill) => {
                let mut width = column.saturating_add(usize::from(*flat_space));
                let mut fits = width <= max_width;
                for (next, _, next_mode, _) in stack.iter().rev() {
                    let prefix = match next_mode {
                        Mode::Flat => next.flat,
                        Mode::Break | Mode::Fill => next.broken,
                    };
                    width = width.saturating_add(prefix.width());
                    if width > max_width {
                        fits = false;
                        break;
                    }
                    if prefix.ends_line() {
                        break;
                    }
                }
                if fits {
                    space = space.max(usize::from(*flat_space && column != 0));
                } else {
                    output.push('\n');
                    column = 0;
                    space = 0;
                }
            }
            Kind::HardLine | Kind::SoftLine(_) => {
                output.push('\n');
                column = 0;
                space = 0;
            }
            Kind::Indent(level, child) => stack.push((
                child,
                level.saturating_add_signed(adjustment),
                mode,
                adjustment,
            )),
            Kind::Group {
                children,
                indent: level,
                fill,
                anchor,
            } => {
                let indent = level.map_or(indent, |level| level.saturating_add_signed(adjustment));
                // Expressions following a statement prefix inherit that
                // physical line's indentation. A RHS on its own line keeps
                // its already-applied continuation indentation.
                let (indent, adjustment) = if *anchor && column != 0 {
                    (
                        line_indent,
                        adjustment + line_indent as isize - indent as isize,
                    )
                } else {
                    (indent, adjustment)
                };
                if *fill {
                    stack.extend(
                        children
                            .iter()
                            .rev()
                            .map(|child| (child, indent, Mode::Fill, adjustment)),
                    );
                    continue;
                }
                let start = if column == 0 {
                    indent.saturating_mul(indent_width)
                } else {
                    column + space
                };
                let measure = doc.flat;
                let mut width = start.saturating_add(measure.width());
                let mut fits = !measure.ends_line() && width <= max_width;
                // Reserve following unbroken text (e.g. `);` or ` then`). Cached
                // summaries skip subtrees; stop at the next break or overflow.
                if fits && !measure.ends_line() {
                    for (next, _, next_mode, _) in stack.iter().rev() {
                        let prefix = match next_mode {
                            Mode::Flat => next.flat,
                            Mode::Break | Mode::Fill => next.broken,
                        };
                        width = width.saturating_add(prefix.width());
                        if width > max_width {
                            fits = false;
                            break;
                        }
                        if prefix.ends_line() {
                            break;
                        }
                    }
                }
                let mode = if fits { Mode::Flat } else { Mode::Break };
                stack.extend(
                    children
                        .iter()
                        .rev()
                        .map(|child| (child, indent, mode, adjustment)),
                );
            }
            Kind::Concat(children) => {
                stack.extend(
                    children
                        .iter()
                        .rev()
                        .map(|child| (child, indent, mode, adjustment)),
                );
            }
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call() -> Doc {
        Doc::group(Doc::concat([
            Doc::text("f("),
            Doc::indent(
                1,
                Doc::concat([
                    Doc::soft_line(false),
                    Doc::text("a,"),
                    Doc::soft_line(true),
                    Doc::text("b"),
                ]),
            ),
            Doc::soft_line(false),
            Doc::text(")"),
        ]))
    }

    #[test]
    fn width_boundary_and_following_suffix() {
        let doc = Doc::concat([call(), Doc::text(";")]);
        assert_eq!(render(&doc, 4, 9), "f(a, b);");
        assert_eq!(render(&doc, 4, 8), "f(a, b);");
        assert_eq!(render(&doc, 4, 7), "f(\n    a,\n    b\n);");
    }

    #[test]
    fn nested_groups_fit_independently() {
        let doc = Doc::group(Doc::concat([
            Doc::text("outer("),
            Doc::hard_line(),
            Doc::indent(1, call()),
            Doc::hard_line(),
            Doc::text(")"),
        ]));
        assert_eq!(render(&doc, 4, 20), "outer(\n    f(a, b)\n)");
    }

    #[test]
    fn indentation_and_spaces_are_lazy() {
        let doc = Doc::concat([
            Doc::text("a"),
            Doc::space(),
            Doc::hard_line(),
            Doc::indent(2, Doc::hard_line()),
            Doc::text("b"),
            Doc::space(),
        ]);
        assert_eq!(render(&doc, 4, 100), "a\n\nb");
    }

    #[test]
    fn multiline_text_is_verbatim() {
        let doc = Doc::indent(
            1,
            Doc::group(Doc::concat([
                Doc::text("/* a\n b */"),
                Doc::soft_line(true),
                Doc::text("x"),
            ])),
        );
        assert_eq!(render(&doc, 4, 100), "    /* a\n b */\n    x");
    }

    #[test]
    fn deeply_nested_groups_use_cached_widths() {
        let mut doc = Doc::text("x");
        for _ in 0..512 {
            doc = Doc::group(Doc::concat([doc]));
        }
        assert_eq!(render(&doc, 4, 1), "x");
    }

    #[test]
    fn statement_prefix_and_nested_call_share_continuation_indent() {
        let doc = Doc::fill_group(Doc::concat([
            Doc::text("x :="),
            Doc::soft_line(true),
            Doc::indent(
                1,
                Doc::expression_group(Doc::concat([
                    Doc::text_at(1, "f(".into()),
                    Doc::soft_line(false),
                    Doc::text_at(2, "aaaa,".into()),
                    Doc::soft_line(true),
                    Doc::text_at(2, "bbbb".into()),
                    Doc::soft_line(false),
                    Doc::text_at(1, ")".into()),
                ])),
            ),
            Doc::text(";"),
        ]));
        assert_eq!(render(&doc, 4, 12), "x := f(\n    aaaa,\n    bbbb\n);");
    }
}

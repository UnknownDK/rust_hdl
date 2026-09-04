// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this file,
// You can obtain one at http://mozilla.org/MPL/2.0/.

use super::{buffer::Buffer, VHDLFormatter};
use crate::{TokenAccess, TokenId, TokenSpan};

impl VHDLFormatter<'_> {
    /// Keep runs local to one syntactic list. Blank lines, comments and
    /// non-alignable items are boundaries; nested lists manage their own runs.
    pub(crate) fn format_aligned_items<T>(
        &self,
        items: &[T],
        buffer: &mut Buffer,
        when_broken: bool,
        target: impl Fn(&T) -> Option<TokenId>,
        span: impl Fn(&T) -> TokenSpan,
        build: impl FnMut(usize, &T, &mut Buffer),
    ) {
        self.format_aligned_items_multi(
            items,
            buffer,
            (when_broken, false),
            |item| target(item).map_or_else(Vec::new, |token| vec![(token, 1)]),
            span,
            build,
        );
    }

    pub(crate) fn format_aligned_items_with_trailing_comments<T>(
        &self,
        items: &[T],
        buffer: &mut Buffer,
        when_broken: bool,
        target: impl Fn(&T) -> Option<TokenId>,
        span: impl Fn(&T) -> TokenSpan,
        build: impl FnMut(usize, &T, &mut Buffer),
    ) {
        self.format_aligned_items_multi(
            items,
            buffer,
            (when_broken, true),
            |item| target(item).map_or_else(Vec::new, |token| vec![(token, 1)]),
            span,
            build,
        );
    }

    pub(crate) fn format_aligned_items_multi<T>(
        &self,
        items: &[T],
        buffer: &mut Buffer,
        settings: (bool, bool),
        targets: impl Fn(&T) -> Vec<(TokenId, usize)>,
        span: impl Fn(&T) -> TokenSpan,
        mut build: impl FnMut(usize, &T, &mut Buffer),
    ) {
        let (when_broken, allow_trailing_comments) = settings;
        let mut rows = Vec::new();
        let mut previous_end = None;
        for (i, item) in items.iter().enumerate() {
            let span = span(item);
            let targets = targets(item);
            let start = self.tokens.index(span.start_token).full_range().start.line;
            let mut interior_comments = false;
            let mut leading_comment = false;
            if !targets.is_empty() {
                for id in span.iter() {
                    if let Some(comments) = &self.tokens.index(id).comments {
                        leading_comment |= id == span.start_token && !comments.leading.is_empty();
                        interior_comments |= ((id != span.end_token || !allow_trailing_comments)
                            && comments.trailing.is_some())
                            || (id != span.start_token && !comments.leading.is_empty());
                    }
                }
            }
            let boundary = targets.is_empty()
                || interior_comments
                || leading_comment
                || previous_end.is_some_and(|end| start > end + 1);
            if boundary {
                buffer.align_rows(&rows, when_broken);
                rows.clear();
            }
            if !targets.is_empty() {
                let index = buffer.alignment_row_targets(&targets, |buffer| build(i, item, buffer));
                if !interior_comments {
                    rows.push(index);
                }
            } else {
                build(i, item, buffer);
            }
            previous_end = Some(self.tokens.index(span.end_token).pos.end().line);
        }
        buffer.align_rows(&rows, when_broken);
    }
}

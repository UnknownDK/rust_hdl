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
        mut build: impl FnMut(usize, &T, &mut Buffer),
    ) {
        let mut rows = Vec::new();
        let mut previous_end = None;
        for (i, item) in items.iter().enumerate() {
            let span = span(item);
            let target = target(item);
            let start = self.tokens.index(span.start_token).full_range().start.line;
            let mut interior_comments = false;
            let comments = target.is_some()
                && span.iter().fold(false, |found, id| {
                    let comments =
                        self.tokens
                            .index(id)
                            .comments
                            .as_ref()
                            .is_some_and(|comments| {
                                interior_comments |= comments.trailing.is_some()
                                    || (id != span.start_token && !comments.leading.is_empty());
                                !comments.leading.is_empty() || comments.trailing.is_some()
                            });
                    found || comments
                });
            let boundary =
                target.is_none() || comments || previous_end.is_some_and(|end| start > end + 1);
            if boundary {
                buffer.align_rows(&rows, when_broken);
                rows.clear();
            }
            if let Some(target) = target {
                let index = buffer.alignment_row(target, |buffer| build(i, item, buffer));
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

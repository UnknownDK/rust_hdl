// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this file,
// You can obtain one at http://mozilla.org/MPL/2.0/.

/// Case used for reserved words, including word operators. Identifiers and
/// literals are never case-normalized.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum KeywordCase {
    #[default]
    Lower,
    Upper,
}

/// Deliberately small set of formatter style options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FormatConfig {
    /// Preferred width in Unicode scalar values. Unbreakable tokens and
    /// preserved comments/disabled regions may exceed it.
    pub max_width: usize,
    /// Spaces per indentation level.
    pub indent_width: usize,
    pub keyword_case: KeywordCase,
}

impl Default for FormatConfig {
    fn default() -> Self {
        Self {
            max_width: 100,
            indent_width: 4,
            keyword_case: KeywordCase::Lower,
        }
    }
}

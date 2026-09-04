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
    /// Align colons in adjacent object/interface declarations.
    pub align_declarations: bool,
    /// Align arrows in adjacent named map and aggregate associations.
    pub align_associations: bool,
    /// Maximum list items to keep inline, when they fit. Zero always expands
    /// nonempty calls, parameter/interface lists, maps and named aggregates.
    /// Purely positional aggregates remain width-driven.
    pub inline_argument_limit: usize,
}

impl Default for FormatConfig {
    fn default() -> Self {
        Self {
            max_width: 100,
            indent_width: 4,
            keyword_case: KeywordCase::Lower,
            align_declarations: false,
            align_associations: false,
            inline_argument_limit: 2,
        }
    }
}

impl FormatConfig {
    /// Read the optional `[format]` section of a project TOML file. Other
    /// project sections are ignored; unknown formatter options are errors.
    pub fn from_toml(text: &str) -> Result<Self, String> {
        let root = text.parse::<toml::Table>().map_err(|err| err.to_string())?;
        let mut config = Self::default();
        let Some(section) = root.get("format") else {
            return Ok(config);
        };
        let table = section.as_table().ok_or("format must be a table")?;
        for (key, value) in table {
            match key.as_str() {
                "max_width" | "indent_width" | "inline_argument_limit" => {
                    let number = value
                        .as_integer()
                        .and_then(|n| usize::try_from(n).ok())
                        .ok_or_else(|| format!("format.{key} must be a nonnegative integer"))?;
                    if key == "max_width" {
                        config.max_width = number;
                    } else if key == "indent_width" {
                        config.indent_width = number;
                    } else {
                        config.inline_argument_limit = number;
                    }
                }
                "keyword_case" => {
                    config.keyword_case = match value.as_str() {
                        Some("lower") => KeywordCase::Lower,
                        Some("upper") => KeywordCase::Upper,
                        _ => return Err("format.keyword_case must be 'lower' or 'upper'".into()),
                    }
                }
                "align_declarations" | "align_associations" => {
                    let enabled = value
                        .as_bool()
                        .ok_or_else(|| format!("format.{key} must be a boolean"))?;
                    if key == "align_declarations" {
                        config.align_declarations = enabled;
                    } else {
                        config.align_associations = enabled;
                    }
                }
                _ => return Err(format!("unknown formatter option format.{key}")),
            }
        }
        config.validate()?;
        Ok(config)
    }

    /// Bound user-supplied settings to avoid accidental huge allocations.
    /// Zero indentation is supported; a line width must be positive.
    pub fn validate(&self) -> Result<(), String> {
        if !(1..=10_000).contains(&self.max_width) {
            return Err("format.max_width must be between 1 and 10000".into());
        }
        if self.indent_width > 32 {
            return Err("format.indent_width must be between 0 and 32".into());
        }
        if self.inline_argument_limit > 10_000 {
            return Err("format.inline_argument_limit must be between 0 and 10000".into());
        }
        Ok(())
    }
}

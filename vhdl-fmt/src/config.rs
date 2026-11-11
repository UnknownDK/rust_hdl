// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this file,
// You can obtain one at http://mozilla.org/MPL/2.0/.

use serde::Deserialize;
use vhdl_syntax::standard::VHDLStandard;

/// Name of the project file whose `[format]` table configures the formatter.
pub const CONFIG_NAME: &str = "vhdl_ls.toml";

/// Case used for reserved words, including word operators. Identifiers and
/// literals are never case-normalized.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum KeywordCase {
    #[default]
    Lower,
    Upper,
}

/// Formatter style options, read from the `[format]` table of `vhdl_ls.toml`.
///
/// The table has the same keys as the `vhdl_lang` formatter, so existing
/// project files keep loading.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FormatConfig {
    /// Preferred line width, in characters. Text that cannot be broken may
    /// exceed it.
    pub max_width: usize,
    /// Spaces per indentation level.
    pub indent_width: usize,
    pub keyword_case: KeywordCase,
    /// Align the colons of adjacent declarations, their `:=`, and the types
    /// after port modes.
    pub align_declarations: bool,
    /// Align the arrows of adjacent named associations.
    pub align_associations: bool,
    /// Align the operators of adjacent unlabeled assignments.
    pub align_assignments: bool,
    /// Line up the comments that end neighbouring lines of code.
    pub align_comments: bool,
    /// Write `name : type` in declarations, not `name: type`. The colon of a
    /// label is never preceded by a space.
    pub space_before_colon: bool,
    /// Most entries a call, map, interface list or named aggregate may have
    /// and still stay on one line. Zero always expands them.
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
            align_assignments: false,
            align_comments: false,
            space_before_colon: false,
            inline_argument_limit: 2,
        }
    }
}

impl FormatConfig {
    /// Bound user-supplied settings to avoid accidental huge allocations.
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

/// Everything the formatter reads from a project file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Settings {
    pub format: FormatConfig,
    pub standard: VHDLStandard,
}

impl Settings {
    /// Read the `standard` key and the `[format]` table of a project file.
    /// Other keys belong to the language server and are ignored; unknown keys
    /// inside `[format]` are errors.
    pub fn from_toml(text: &str) -> Result<Self, String> {
        let root = text.parse::<toml::Table>().map_err(|err| err.to_string())?;
        let mut settings = Self::default();
        if let Some(value) = root.get("standard") {
            settings.standard = value
                .as_str()
                .and_then(|standard| standard.parse().ok())
                .ok_or("standard must be a VHDL revision such as \"2008\"")?;
        }
        if let Some(section) = root.get("format") {
            settings.format = section
                .clone()
                .try_into()
                .map_err(|err: toml::de::Error| format!("format: {}", err.message()))?;
            settings.format.validate()?;
        }
        Ok(settings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_format_table_and_ignores_other_sections() {
        let settings = Settings::from_toml(
            "standard = \"1993\"\n[libraries]\nlib.files = []\n[format]\nkeyword_case = \"upper\"\nindent_width = 2\n",
        )
        .unwrap();
        assert_eq!(settings.standard, VHDLStandard::VHDL1993);
        assert_eq!(settings.format.keyword_case, KeywordCase::Upper);
        assert_eq!(settings.format.indent_width, 2);
        assert_eq!(settings.format.max_width, 100);
    }

    #[test]
    fn a_missing_table_means_defaults() {
        assert_eq!(Settings::from_toml("").unwrap(), Settings::default());
    }

    #[test]
    fn rejects_unknown_and_invalid_options() {
        for text in [
            "[format]\nwidth = 3\n",
            "[format]\nmax_width = 0\n",
            "[format]\nkeyword_case = \"title\"\n",
            "standard = 2008\n",
        ] {
            assert!(Settings::from_toml(text).is_err(), "{text}");
        }
    }
}

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this file,
// You can obtain one at http://mozilla.org/MPL/2.0/.

//! Replace disabled source with opaque comment anchors before parsing. The
//! anchors participate in normal comment/token verification; afterwards the
//! original bytes replace each anchor. Disabled contents need not be valid VHDL.

pub(super) struct DisabledRegions {
    pub masked: String,
    regions: Vec<(String, String, bool)>,
    /// (masked start/end, original start/end), in UTF-8 bytes.
    offsets: Vec<(usize, usize, usize, usize)>,
}

impl DisabledRegions {
    pub fn new(text: &str, standard: crate::VHDLStandard) -> Self {
        if !text.contains("vhdl_ls off") {
            return Self {
                masked: String::new(),
                regions: Vec::new(),
                offsets: Vec::new(),
            };
        }
        let mut prefix = "__rust_hdl_disabled_region_".to_owned();
        while text.contains(&prefix) {
            prefix.push('_');
        }
        let mut result = Self {
            masked: String::new(),
            regions: Vec::new(),
            offsets: Vec::new(),
        };
        let bytes = text.as_bytes();
        let mut copied = 0;
        let mut i = 0;
        let mut can_be_char = true;
        while i < bytes.len() {
            let comment = if bytes[i..].starts_with(b"--") {
                let end = text[i..].find('\n').map_or(text.len(), |n| i + n);
                Some((i + 2, end, end))
            } else if bytes[i..].starts_with(b"/*") {
                let Some(length) = text[i + 2..].find("*/") else {
                    break;
                };
                let end = i + 2 + length;
                Some((i + 2, end, (end + 2).min(text.len())))
            } else {
                None
            };
            if let Some((body, end, after)) = comment {
                if text[body..end].trim() == "vhdl_ls off" {
                    let line_start = text[..i].rfind('\n').map_or(0, |n| n + 1);
                    let whole_line = text[line_start..i].trim().is_empty();
                    let start = if whole_line { line_start } else { i };
                    let finish = disabled_end(text, after);
                    let marker = format!("--{prefix}{}__", result.regions.len());
                    result.masked.push_str(&text[copied..start]);
                    let masked_start = result.masked.len();
                    result.masked.push_str(&marker);
                    result.masked.push('\n');
                    result
                        .offsets
                        .push((masked_start, result.masked.len(), start, finish));
                    result
                        .regions
                        .push((marker, text[start..finish].to_owned(), whole_line));
                    copied = finish;
                    i = finish;
                } else {
                    i = after;
                }
                continue;
            }
            // Do not interpret comment delimiters inside strings, extended
            // identifiers, or character literals as formatter directives.
            if bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' || bytes[i] >= 0x80 {
                let start = i;
                i += 1;
                while i < bytes.len()
                    && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' || bytes[i] >= 0x80)
                {
                    i += 1;
                }
                let word = &text[start..i];
                // As in the tokenizer's IR1045 rule, an apostrophe after a
                // name is a qualification/attribute tick, not a character.
                // In particular, character'('"') must not hide later directives.
                can_be_char = bytes[start].is_ascii_digit()
                    || standard.keywords().iter().any(|kind| {
                        *kind != crate::syntax::Kind::All
                            && word.eq_ignore_ascii_case(crate::kind_str(*kind))
                    });
            } else if bytes[i] == b'\'' && can_be_char {
                let character_end = text[i + 1..].chars().next().map(|ch| i + 1 + ch.len_utf8());
                if let Some(end) = character_end.filter(|&end| bytes.get(end) == Some(&b'\'')) {
                    i = end + 1;
                } else {
                    i += 1;
                }
            } else if matches!(bytes[i], b'"' | b'\\') {
                let delimiter = bytes[i];
                can_be_char = delimiter != b'\\';
                i += 1;
                while i < bytes.len() {
                    if bytes[i] == delimiter {
                        i += 1;
                        if bytes.get(i) != Some(&delimiter) {
                            break;
                        }
                    }
                    i += 1;
                }
            } else {
                if !bytes[i].is_ascii_whitespace() {
                    can_be_char = !matches!(bytes[i], b')' | b']');
                }
                i += 1;
            }
        }
        result.masked.push_str(&text[copied..]);
        result
    }

    pub fn is_empty(&self) -> bool {
        self.regions.is_empty()
    }

    /// Diagnostics for enabled code still refer to the user's original source,
    /// even though whole regions were replaced by single-line parser anchors.
    pub fn remap_diagnostics(
        &self,
        diagnostics: &mut [crate::Diagnostic],
        source: &crate::Source,
        text: &str,
    ) {
        if self.is_empty() {
            return;
        }
        let map_position = |pos: crate::Position| {
            let mut offset = 0;
            for (line_number, line) in self.masked.split_inclusive('\n').enumerate() {
                if line_number == pos.line as usize {
                    let mut column = 0;
                    for ch in line.chars() {
                        if column >= pos.character {
                            break;
                        }
                        column += ch.len_utf16() as u32;
                        offset += ch.len_utf8();
                    }
                    break;
                }
                offset += line.len();
            }
            let mut original = offset;
            for &(start, end, original_start, original_end) in &self.offsets {
                if offset < start {
                    break;
                }
                if offset < end {
                    original = original_start;
                    break;
                }
                original = original_end + offset - end;
            }
            let prefix = &text[..original.min(text.len())];
            let line = prefix.bytes().filter(|&b| b == b'\n').count() as u32;
            let last_line = prefix.rsplit('\n').next().unwrap_or("");
            crate::Position {
                line,
                character: last_line.encode_utf16().count() as u32,
            }
        };
        for diagnostic in diagnostics {
            diagnostic.pos = source.pos(
                map_position(diagnostic.pos.start()),
                map_position(diagnostic.pos.end()),
            );
            for (pos, _) in &mut diagnostic.related {
                *pos = source.pos(map_position(pos.start()), map_position(pos.end()));
            }
        }
    }

    pub fn restore(&self, formatted: &str) -> Option<String> {
        let mut output = String::new();
        let mut rest = formatted;
        for (marker, original, whole_line) in &self.regions {
            let index = rest.find(marker)?;
            let line_start = rest[..index].rfind('\n').map_or(0, |n| n + 1);
            let prefix_end = if *whole_line && rest[line_start..index].trim().is_empty() {
                line_start
            } else {
                index
            };
            output.push_str(&rest[..prefix_end]);
            output.push_str(original);
            let end = index + marker.len();
            // Each synthetic anchor is a line comment. Its generated newline
            // is part of the placeholder, not part of the enabled source.
            rest = rest.get(end..)?.strip_prefix('\n')?;
        }
        output.push_str(rest);
        Some(output)
    }
}

fn disabled_end(text: &str, start: usize) -> usize {
    let mut scanner = crate::syntax::IgnoredRegionEnd::default();
    for (offset, ch) in text[start..].char_indices() {
        if scanner.push(ch) {
            let end = start + offset + ch.len_utf8();
            // Include the line ending immediately after a block directive in
            // the preserved region, just as for a line-comment directive.
            return end
                + if ch == '\n' {
                    0
                } else if text[end..].starts_with("\r\n") {
                    2
                } else if text[end..].starts_with('\n') {
                    1
                } else {
                    0
                };
        }
    }
    text.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markers_inside_literals_and_comments_are_not_directives() {
        for input in [
            r#""-- vhdl_ls off""#,
            "-- mention vhdl_ls off\nx",
            "/*\n-- vhdl_ls off\n*/",
            r#"\-- vhdl_ls off\"#,
        ] {
            assert!(
                DisabledRegions::new(input, crate::VHDLStandard::default()).is_empty(),
                "{input}"
            );
        }
    }

    #[test]
    fn opaque_regions_round_trip_exactly() {
        for input in [
            "  --vhdl_ls off\n  invalid € \"\n--vhdl_ls on\n",
            "-- vhdl_ls off\r\ninvalid\r\n-- vhdl_ls on\r\n",
            "-- vhdl_ls off\nno ending newline",
        ] {
            let regions = DisabledRegions::new(input, crate::VHDLStandard::default());
            assert_eq!(regions.restore(&regions.masked).unwrap(), input);
        }
    }
}

# rust_hdl — VHDL formatter fork

Beware - AI has been used heavily, since I don't know rust.


This fork of [VHDL-LS/rust_hdl](https://github.com/VHDL-LS/rust_hdl) adds an
experimental, configurable VHDL formatter to the `vhdl_lang` CLI and library.
It supports width-aware wrapping, lower/uppercase keywords, argument-count
wrapping and optional alignment, with token and comment preservation checks.
See [VHDL formatter](#vhdl-formatter) for building, configuration and usage.

## VHDL formatter

The experimental formatter preserves identifiers, literals, comments and token
order while applying consistent layout and width-aware wrapping. It reparses and
verifies the result before writing anything to stdout; errors go to stderr.

```sh
cargo build --release -p vhdl_lang --bin vhdl_lang
target/release/vhdl_lang --format design.vhd
target/release/vhdl_lang --format-stdin --stdin-filepath design.vhd --max-width 100 --indent-width 4 --keyword-case upper
```

Both modes write to stdout, without modifying the input file. For an external
editor formatter, use `--format-stdin` and pass the document path through
`--stdin-filepath` for diagnostics and project configuration discovery. The CLI
defaults to VHDL-2008; `--standard 1993|2008|2019` overrides the project standard.
The library API accepts a parser configured for any of those standards.

Both file and stdin input default to UTF-8. For legacy ISO-8859-1 input, pass
`--input-encoding latin1` explicitly; invalid UTF-8 otherwise fails without
formatted output. Output is always UTF-8, including when Latin-1 input is
selected. This replaces the formatter's previous implicit Latin-1 file decoding
and makes file and stdin behavior consistent. It does not change the language
server's source-loading policy.

The defaults are `max_width = 100`, `indent_width = 4` and
`keyword_case = lower`. Keyword case supports `lower` and `upper`, including word
operators such as `and` and `not`; identifiers, quoted operator symbols and
literals are unaffected. Width counts Unicode scalar values. Unbreakable tokens,
preserved comments and disabled regions may exceed the preferred width.

To enable selective alignment (declaration colons and map/aggregate arrows, but
not default values), add this section to your project's `vhdl_ls.toml`:

```toml
[format]
max_width = 100
indent_width = 4
keyword_case = "lower"
align_declarations = true
align_associations = true
inline_argument_limit = 2
```

The formatter uses the nearest `vhdl_ls.toml` in the input file's directory or
its ancestors. Stdin uses `--stdin-filepath`, including for unsaved files; without
that flag it searches from the working directory. The nearest file is used on
its own, without merging parent files. Existing Zed stdin commands therefore
pick up project settings without additional formatter flags. Library APIs do
not discover files implicitly; use `FormatConfig::from_toml` to load settings.

CLI options override project settings. Use `--format-config path/to/settings.toml`
to select a file explicitly, or `--no-format-config` to disable discovery.
`--align-declarations` and `--align-associations` enable alignment;
`--align-declarations=false` and `--align-associations=false` disable it. Both
default to false. Width must be 1–10000, indentation 0–32 spaces. Unknown options
in `[format]`, invalid values, and unreadable selected configuration files are
errors reported on stderr without formatted output.

`inline_argument_limit` is shared by calls, function/procedure parameters,
generic/port maps, generic/port interface lists, and aggregates containing named
associations. By default, up to two arguments stay on one line **if they fit**;
three or more expand. Use `0` to
always expand nonempty lists, or a larger value to allow more inline arguments.
Override it with `--inline-argument-limit N` (0–10000). Width and comments can
still force shorter lists to wrap. Grouped parameter names count individually,
but their existing declaration grouping is retained. Because calls and indexing
are syntactically ambiguous, indexed names use the same count rule; slices and
ordinary parenthesized expressions are unaffected.

Named aggregates use the same limit, counting each association once (including
positional entries in a mixed aggregate). Purely positional aggregates remain
width-driven. Lists made entirely of simple scalar values pack as many complete
values on each line as `max_width` allows, which keeps large lookup tables dense.
A complex or named item gives the aggregate its structural one-item-per-line
layout. Aggregate targets also remain structural. At the default limit,
`(others => '0')` stays compact; `0` expands even single-association named
aggregates. Nested aggregates choose their layouts independently.

Expanded function headers keep `) return ... is` together when it fits.
Assert `report` and `severity` clauses always start on their own indented lines,
independently of the argument limit:

```vhdl
file_open(
    foo,
    bar,
    read_mode
);
assert false
    report "Unsupported bla bla"
    severity failure;
```

Colons align in adjacent object/file declarations, interfaces and record fields.
Arrows align in named port/generic map and aggregate associations; ordinary calls
retain their spacing. Inline lists never receive column padding. Interfaces and
maps align whenever they expand, whether because of width, comments or the
argument limit. Named aggregates align when their item count exceeds the inline limit.
Blank lines, standalone comments and other item kinds separate
groups. Rows with internal/trailing comments or wrapped content do not contribute
padding. Alignment falls back to ordinary spacing if the combined columns would
exceed the configured width. Modes, types, defaults and assignments are not
separately aligned.

Expressions prefer breaks between logical operands over breaks inside their
comparisons, and between arithmetic terms over breaks inside their products.
Expanded `if`/`elsif` headers put `then` on its own line; short headers stay inline.
Expanded `while`/`for` loops and generate headers do the same with `loop` and
`generate`. Long `case` and selected-assignment choice lists wrap before `|`,
keeping each choice intact when possible.
Operators lead continuation lines, with parentheses and token order preserved:

```vhdl
if input_valid = '1'
    and output_ready = '1'
    and transfer_enabled = '1'
then
    header <= (
        valid  => '1',
        opcode => OP_WRITE,
        length => payload_length
    );
end if;
```

The example assumes the condition needs wrapping and association alignment is
enabled. Comparisons can still wrap if they cannot fit by themselves.

Grouped interface names wrap at one indentation level. A long declaration
prefers moving its complete mode/type to the next line before splitting the
type's range; internal wrapping remains available if the type cannot fit there
either. Keyword-prefixed declarations retain their continuation indentation.

Long attribute specifications break after `of` and `is`. Aliases separate their
type and target at clause boundaries, keeping complete type constraints together
when they fit. Short attributes and aliases stay inline. Long external names
expand inside `<< ... >>`, preserving the path and preferring a complete subtype
on a continuation line.

Enumeration literals and array dimensions use width-aware lists: short lists
stay inline, while expanded lists put each item on its own line. These lists do
not use `inline_argument_limit`. Array declarations keep `) of <type>` together
when possible, moving the complete element type below `of` before splitting its
constraint.

Simple timed or multi-element signal assignments stay inline when they fit.
When expanded, their waveform starts below the assignment operator with one
element per line. Each value stays beside its `after` delay when possible;
otherwise the timing clause uses a further-indented continuation:

```vhdl
output_signal <=
    first_long_value after first_long_delay,
    second_long_value after second_long_delay;
output_signal <=
    calculate_output(input_data, settings)
        after PROPAGATION_DELAY;
```

Expanded selected assignments separate the selector from the indented target,
keeping the first value beside that target when it fits:

```vhdl
with selection_value select
    result <= first_value when first_choice,
        second_value when second_choice,
        default_value when others;
```

Short selected assignments remain inline. The same rules apply to concurrent
signal assignments and sequential signal, variable and force assignments.

One blank line separates context clauses from their design unit, processes from
neighboring concurrent statements, and subprogram bodies from neighboring
declarations. Existing declaration groups are retained and extra blank lines
are collapsed. Leading comments remain attached to the following block.

Use `format_source` for the default library API, or
`format_source_with_config(&parser, &source, &FormatConfig { ... })` to configure
it. `FormatConfig` and `KeywordCase` are re-exported by `vhdl_lang`. When starting
from raw text, `format_text_with_config(&parser, path, text, &config)` also
preserves original disabled-region line endings before `Source` normalizes them.
The CLI uses this raw-text entry point. The lower-level AST-only
`VHDLFormatter::format_design_file` does not provide source-preservation checks.

**Warning:** `vhdl_ls off/on` disables parsing and language-server analysis of
the enclosed text as well as formatting. It is not a formatter-only ignore
mechanism; declarations and references in that region are invisible to analysis.

Code between exact `-- vhdl_ls off` and `-- vhdl_ls on` directives remains
unchanged while surrounding code is formatted. An unmatched `off` extends to
EOF. Trailing directives and block-comment directives are also supported;
an `off` inside a string, identifier or ordinary comment prose does not start a
region. Within a disabled region, only comment boundaries are scanned to find
`on`; strings and other source syntax are not validated. Disabled text need not
be valid VHDL.

```vhdl
-- vhdl_ls off
-- This region is preserved verbatim and excluded from analysis.
-- vhdl_ls on
```

Directive names are case-sensitive. A block comment can span multiple lines,
but its entire trimmed contents must equal `vhdl_ls off` or `vhdl_ls on`.

See [formatter tests and implementation notes](vhdl_lang/tests/formatting/README.md)
for layout policy, corpus coverage and benchmark commands. Identifier case
normalization, tabs and VSG compatibility are not implemented.

## Building the project locally

1) Make sure that you have the [Rust toolchain](https://www.rust-lang.org/tools/install) installed.
   This repository always follows the latest toolchain in the `stable` channel.
2) Run `cargo install --path vhdl_lang` to install the language frontend. Run instead `cargo install --path vhdl_ls`
   to install the language server.
3) Make sure that the default libraries are available at a visible path. Search paths are, for example,
   `/usr/lib/rust_hdl/vhdl_libraries` or `/usr/local/lib/rust_hdl/vhdl_libraries`.
4) Run the command `vhdl_lang` or `vhdl_ls` to run the language front-end binary or the language server

**Testing the Language Server**

For checking the language server, [rust_hdl_vscode](https://github.com/VHDL-LS/rust_hdl_vscode) is recommended.
To instruct the extension to use the new binary, instead of a downloaded one, go to the extension settings and set
the Language server location to `systemPath`. To specify the exact path, set it to `user` and set Language Server User
Path to the path that points to the `vhdl_ls` binary.

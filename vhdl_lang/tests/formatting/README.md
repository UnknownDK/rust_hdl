# Formatter layout and validation

The formatter builds one nested document for each complete file. AST visitors
continue to use `Buffer`, but it is now a document builder, not a rendered string
sink. There is no captured child-buffer rendering. Text, hard lines, soft lines,
empty-or-newline breaks, indentation, groups and concatenations compose before
the final render.

Argument lists flatten only when they fit and their count is at most
`inline_argument_limit` (default 2). Above that count, each list item gets its own
line even at wide line widths. The rule covers calls/indexed names, subprogram
parameters, generic/port maps and generic/port interfaces. Grouped names in an
interface declaration count individually without changing the grouping's tokens.
Empty lists remain compact; zero forces all nonempty lists to expand. Slices,
aggregates, sensitivity lists and ordinary parenthesized expressions keep their
existing width-aware rules. Nested groups choose independently. Statement continuation groups decide
their spaces/breaks locally, so an assignment prefix can remain beside a call
whose arguments wrap. Expressions inherit the indentation of the physical line
where they start; their closing delimiters return to that level. Multi-item
port/generic maps and interface lists follow the shared argument limit. Binary operators
lead continuation lines; the left spine of a binary-expression tree does not
produce increasing indentation. Existing parentheses are retained.

Function headers use local continuation decisions so an expanded parameter list
does not force `return` onto a new line after `)`. The return type and `is` remain
beside `)` when they fit. Assertions always put `report` and `severity` on separate
continuation lines; an assertion with neither clause stays compact. Standalone
report statements retain their existing width-aware behavior.

Width summaries are cached during document construction. The renderer uses an
explicit stack and bounded lookahead through cached summaries, rather than
repeatedly rendering or measuring nested subtrees. Indentation and formatter
spaces are emitted lazily, so blank lines have no generated trailing spaces.
Original spaces within comment text and disabled regions remain untouched.

## Selective alignment

`align_declarations` and `align_associations` are opt-in. The former aligns
colons in object/file declarations, interfaces and record fields; the latter
aligns named port/generic map arrows. It does not align modes, types, default
expressions, assignments, ordinary calls or aggregates.
Inline lists do not receive alignment padding. In interfaces and maps, column
alignment applies to lists expanded by the argument limit; short lists that wrap
only for width/comments retain ordinary spacing.

Rows carry an `Align` document primitive, not literal spaces in token text.
Adjacent single-line rows are measured using document width summaries. The
widest prefix and suffix determine whether the complete run fits. If not, the
run keeps ordinary spacing. Wrapped rows are excluded and split runs, so a long
declaration cannot force neighboring declarations to wrap. Blank lines,
standalone comments, positional associations and other declaration kinds are
also boundaries. A standalone leading comment starts a new group with its
following declaration; rows with internal or trailing comments are excluded.
Padding is lazy and summaries are refreshed before enclosing groups are built.

`alignment_rtl.input.vhd` and `alignment_rtl.expected.vhd` provide a reviewed,
synthetic RTL style baseline with a state machine, payload registers, interface
groups and named maps. `format_alignment.rs` checks this golden output and
exercises both casing options, width fallback, nesting, mixed associations,
comment boundaries, disabled sections and hundreds of width/indent combinations.

## Project settings

The CLI discovers the nearest ancestor `vhdl_ls.toml` starting at the input
file's directory (or `--stdin-filepath` for stdin, otherwise the working
directory). Discovery supports unsaved files. Only the nearest configuration
is used; parent configurations are not merged. `[format]` settings override
defaults and CLI flags override those settings. The top-level `standard` selects
the parser version, overridden by `--standard`.

`--format-config` explicitly selects a file; `--no-format-config` skips discovery.
`inline_argument_limit` and `--inline-argument-limit` accept 0–10000.
Use `--align-declarations=false` or `--align-associations=false` to override a
project's enabled alignment. Invalid configuration fails with exit code 2 and
no stdout. `[format]` rejects unknown keys. The formatter loader does not require
`[libraries]`; a shared language-server configuration still requires that section.
Library APIs do not implicitly read the filesystem; `FormatConfig::from_toml`
loads the formatter section explicitly.

## Safety checks

`format_source` and `format_source_with_config` parse the input and output and
check design-unit/token counts, token kinds, exact non-keyword source spelling,
comment text/order, and each comment's gap between significant tokens. Only
reserved-word case conversion is allowed. The tests also assert idempotency.

Disabled sections are temporarily represented by unique comment anchors. After
enabled text passes verification, the exact original region replaces its
anchor. The tokenizer skips disabled contents without trying to lex malformed
strings or invalid characters. Diagnostics are mapped back to original source
positions. Directive names are case-sensitive, matching the tokenizer's policy.
Both line and multiline block comments support directives; only the entire
trimmed comment contents count, not a mention embedded in prose. Disabled text
is also excluded from parsing and language-server analysis, not just formatting.
Declarations and references inside it are therefore invisible to analysis.

`Source` already normalizes newlines. To preserve raw CRLF text in disabled
sections, use `format_text_with_config`; the CLI does so automatically. Enabled
text is normalized to LF. Both file and stdin input default to UTF-8, reject
invalid bytes without stdout, and accept legacy ISO-8859-1 only with explicit
`--input-encoding latin1`. Output is always UTF-8. CLI tests exercise non-ASCII
literals/comments and both encodings in both input modes.

## Tests and corpus

```sh
cargo test --workspace
cargo clippy -p vhdl_lang --all-targets -- -D warnings
```

`format_wrapping.rs` covers width boundaries, many widths/indent sizes, nested
calls/aggregates, assignments, declarations, conditionals, selections, assertions,
reports, waits, returns, VHDL-2019 views, comment break points, Unicode, both
keyword cases and lossless disabled sections. Its deterministic generative tests
vary whitespace in valid token streams and insert comments at every token gap.
Renderer unit tests include deeply nested groups and suffix-width accounting.
`format_arguments.rs` checks the shared threshold across constructs, exact
function/assert layouts, grouped declarations, nested calls/indexing, comments,
width fallback and idempotency. Configuration tests cover TOML loading and CLI
overrides for the threshold in both file and stdin modes.

`format_corpus.rs` uses six version-controlled IEEE sources, each carrying an
Apache-2.0 notice under `tests/formatting/corpus/ieee2008` within the crate.
[Corpus provenance and checksums](corpus/README.md) identify the unchanged source
snapshots; they are included in Cargo source packages:

- `math_real.vhdl` and `math_real-body.vhdl`: VHDL-1993, 2008 and 2019.
- `std_logic_1164.vhdl`, `std_logic_1164-body.vhdl`, `numeric_std.vhdl` and
  `numeric_std-body.vhdl`: VHDL-2008 and 2019.

Every applicable version is tested in both keyword cases, with alignment off
and on: 56 file/version/case/alignment combinations, each checked for preservation
and idempotency. Compile-time file
inclusion prevents an empty corpus from silently passing. The separate
`format_example_project` test covers optional populated example-project
submodules using the same verified API and reports the number of files checked.
It explicitly reports a skip when the example-project directory is absent.
Golden fixtures and corpus snapshots are pinned to LF by `.gitattributes`, so
Windows checkouts do not change the expected strings or benchmark inputs.
The bundled corpus emphasizes library declarations and algorithmic bodies;
synthetic RTL fixtures cover processes, instantiations and generate structures.

## Performance and editor smoke test

```sh
cargo bench -p vhdl_lang --bench formatter
python3 vhdl_lang/benches/compare_formatter.py BASELINE_BINARY CURRENT_BINARY
```

Use equally optimized binaries and run comparisons while no other build is
active. The comparison script uses two warm-ups and ten measured runs per input,
reports medians, and disables project configuration discovery. Both binaries
must support `--no-format-config`. Record their exact Git commits, `rustc -Vv`,
build profile, CPU/OS and benchmark command with any published measurements.
The in-process benchmark includes both parsing passes and safety checks;
the CLI comparison also includes process startup and file I/O. Absolute timings
depend on the host. The development target is less than 20% latency regression
per milestone, prioritizing correctness.

For a manual Zed smoke test, configure the external formatter to invoke the
locally built `vhdl_lang` with `--format-stdin --stdin-filepath {buffer_path}`.
Check a valid file with project settings, an unsaved buffer, a file with
non-ASCII comments, and invalid VHDL. Invalid input should report an error without
replacing the buffer. A second save of valid formatted input must make no changes.
Configuration discovery and stdin/API parity are exercised automatically in
`format_config.rs` and `format_cli.rs`; repeat these live checks when changing
editor integration.

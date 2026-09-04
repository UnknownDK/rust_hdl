# Formatter layout and validation

The formatter builds one nested document for each complete file. AST visitors
continue to use `Buffer`, but it is now a document builder, not a rendered string
sink. There is no captured child-buffer rendering. Text, hard lines, soft lines,
empty-or-newline breaks, indentation, groups and concatenations compose before
the final render.

Argument lists flatten only when they fit and their count is at most
`inline_argument_limit` (default 2). Above that count, each list item gets its own
line even at wide line widths. The rule covers calls/indexed names, subprogram
parameters, generic/port maps, generic/port interfaces and aggregates containing
named associations. Each aggregate association counts once, including positional
entries in mixed aggregates. Grouped names in an interface declaration count
individually without changing the grouping's tokens. Empty lists remain compact;
zero forces all nonempty covered lists to expand. Slices, sensitivity lists and
ordinary parenthesized expressions keep their existing width-aware rules. Purely
positional aggregates are also width-driven: all-scalar value lists use a fill
layout that packs complete values greedily, while a named or complex value keeps
the whole aggregate structural. Unary signs in packed values are atomic with
their operands. Targets never use value packing. Nested groups choose
independently. Statement
continuation groups decide their spaces/breaks locally, so an assignment prefix can remain beside a call
whose arguments wrap. Expressions inherit the indentation of the physical line
where they start; their closing delimiters return to that level. Multi-item
port/generic maps and interface lists follow the shared argument limit. Binary
operators lead continuation lines. Same-precedence operators share a layout group along
the left spine, without staircase indentation. Tighter operands get independent
groups, so wrapping a logical chain does not unnecessarily split a comparison,
and wrapping a sum does not split products that fit. Concatenations also keep
arithmetic operands in separate groups. Existing parentheses are retained.
`if`/`elsif` headers group their condition with a break before `then`: expanded
headers place `then` at the statement indentation; short headers remain inline.
Loop and generate headers use the same grouped break before `loop`/`generate`.
Labels remain attached to their header, and unconditional loops stay compact.

Grouped interface and record names share one indentation level when wrapped;
declarations with an explicit keyword keep continuation indentation after that
keyword. A preferred break before a mode/type probes the following group's
cached flat width. If the complete group fits on a continuation line, that break
is preferred to splitting its range. Otherwise the existing internal breaks are
used. Defaults remain separate groups, so a long default does not unnecessarily
expand a short type. No child document is rendered during this decision.

Attribute declarations wrap before their type; expanded specifications break
after `of` and `is`, with independent wrapping of the entity/class clause and
value. Aliases group breaks before their subtype and target, retaining complete
type constraints when they fit with the following `is`. External names use an
independently grouped `<< ... >>` layout with a preferred break before their
subtype. Absolute, relative and package paths, signatures and comments retain
their original tokens and anchors. Short declarations and external names remain
inline.

Enumeration literals and array dimensions are width-aware delimiter groups,
independent of `inline_argument_limit`. Expanded lists break after the opening
parenthesis, between items and before the closing parenthesis. Dimensions retain
their own range/expression wrapping. Array element types use a preferred break
after `of`, keeping the complete type together before splitting its constraint.

Simple timed or multi-element signal assignments group the break after `<=`
with their waveform separators. When expanded, the first element and all later
elements start on separate continuation lines. Short waveforms remain inline;
untimed single-expression assignments retain their existing local wrapping.
Concurrent and sequential signal assignments share this implementation. Each
waveform element independently groups its value and delay, breaking before
`after` at one further indentation level when necessary. There is no optional
break immediately after `after`; delay expressions can still wrap internally,
and a comment can require a line break. An indivisible timing clause may exceed
very narrow widths. Selected and conditional assignments retain their existing
branch layout while using the same element-level timing rules.

Selected assignments group `with ... select` and their body together. When the
statement expands, its target starts on a new indented line; the first value
stays beside the target when possible and later alternatives use continuation
indentation. The shared layout handles concurrent signals and sequential
variables, signals and force assignments, preserving matching selections and
postponed prefixes. Choice lists share an operator-leading `|` layout between
selected assignments, case statements and case generates, with independent
wrapping of discrete ranges and expressions inside each choice.

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
aligns named port/generic map and aggregate arrows. It does not align modes,
types, default expressions, assignments or ordinary calls.
Inline lists do not receive alignment padding. Interfaces and maps align when
expanded by width, comments, preserved blank lines or the argument limit. Named
aggregates retain their argument-limit-based alignment policy.

For interfaces/maps below the argument limit, only rows needing extra padding
have separate unpadded and padded document branches. The enclosing list measures
the unpadded flat branch, then selects the padded branch if it expands. Padding
therefore cannot force an otherwise-fitting list to wrap. Both branches cache
their width summaries; no child rendering or second AST formatting pass is used.
Ordinary newlines between items do not force expansion; actual blank lines are
preserved as alignment boundaries. Separator comments are included when deciding
whether a row can participate.

Rows carry an `Align` document primitive, not literal spaces in token text.
Adjacent single-line rows are measured using document width summaries. The
widest prefix and suffix determine whether the complete run fits. If not, the
run keeps ordinary spacing. Wrapped rows are excluded and split runs, so a long
declaration cannot force neighboring declarations to wrap. Blank lines,
standalone comments, positional associations and other declaration kinds are
also boundaries. A standalone leading comment starts a new group with its
following declaration; rows with internal or trailing comments are excluded.
Padding is lazy and summaries are refreshed before enclosing groups are built.
Aggregate comma comments participate in boundary detection; nested multiline
values split alignment runs while small nested aggregates can stay inline.

## Structural spacing

Context clauses are separated from their design unit by one blank line; context
declaration bodies retain ordinary spacing. A process is separated from each
neighboring concurrent statement, and a subprogram body from each neighboring
declaration, by one blank line. No extra blank line is added at list edges.
Existing declaration groups are preserved, multiple empty lines collapse to one,
and separation is inserted before leading documentation comments. Disabled
regions and comment contents retain their preservation guarantees.

`readability.input.vhd` and `readability.expected.vhd` illustrate these rules at
width 60 with association alignment enabled. `format_readability.rs` checks
exact layouts, language versions, comment and positional boundaries, nesting,
width fallback and preservation/idempotency across 180 option combinations.

`positional_aggregates.input.vhd` and `positional_aggregates.expected.vhd` show
a packed numeric lookup table beside a structural complex aggregate at width 60
and two-space indentation. `format_positional_aggregates.rs` checks exact output,
width filling, atomic signs, scalar names, named/mixed/complex and nested values,
qualified aggregates, structural targets, comment breaks, disabled regions and
all language versions. It also checks comment preservation and idempotency across
144 width/indent/argument-limit/casing/alignment combinations.

`layout_consistency.input.vhd` and `layout_consistency.expected.vhd` show grouped
names, complete types, block headers and selected assignments at width 60 with
declaration alignment enabled. `format_layout_consistency.rs` checks the exact
output, short and narrow layouts, selection modifiers and language versions,
plus comment preservation/idempotency across 144 option combinations. Renderer
unit tests cover the preferred break's flat, whole-type and internal-wrap cases.

`types_and_waveforms.input.vhd` and `types_and_waveforms.expected.vhd` show
enumerations, array dimensions and timed waveforms at width 60.
`format_types_and_waveforms.rs` checks these golden layouts, exact width
boundaries, short forms, constrained dimensions, whole-type and narrow-width
fallbacks, timing modifiers, selected/conditional waveforms and all three
language standards. Comment anchoring and idempotency are exercised across 144
width/indent/argument-limit/casing/alignment combinations.

`declaration_details.input.vhd` and `declaration_details.expected.vhd` show
attribute/alias clauses, an external name and two-item aligned interfaces/maps
at width 60. `format_declaration_details.rs` covers the golden layout, inline and
width-boundary behavior, alignment fallback and boundaries, narrow declarations,
signatures, external-name paths, disabled regions and language versions. Its
comment/nesting matrix checks 288 width/indent/argument-limit/casing/alignment
combinations. A renderer unit test verifies that conditional padding cannot
force an inline list to wrap.

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

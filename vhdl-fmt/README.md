# vhdl-fmt

A VHDL formatter built on the lossless syntax tree of
[`vhdl_syntax`](../vhdl_syntax). It is the successor of the formatter in
`vhdl_lang`, which prints from an AST and therefore loses any token the AST
does not record.

## Status

The formatter decides the block structure of a file:

- every declaration and statement starts its own line, indented by nesting;
- `begin`, `elsif`, `else`, `when` alternatives and `end ...` start a line at
  the level of the construct they belong to;
- use clauses are indented below their library clause, and generic and port
  maps below the instantiation they belong to;
- design units are separated by a blank line.

Lines are laid out to fit `max_width`; the source's own line breaks are not
kept.

- Parenthesized lists stay on one line if they fit: generic and port clauses
  and maps, subprogram parameters, calls and indexed names, aggregates,
  enumeration literals and sensitivity lists. Otherwise every entry gets its
  own line. Interface lists, maps and named aggregates are also expanded when
  they have more than `inline_argument_limit` entries. Short positional values
  are packed several to a line.
- Expressions break before their operators, and operators of one kind break
  together.
- Assignments and initial values move to the next line after `<=` or `:=`.
  Conditional and selected assignments break before each `else` or after each
  `,`.
- `if`, loop and generate headers break before `then`, `loop` or `generate`.
  `wait` and `report` break before their clauses; an assertion always puts
  `report` and `severity` on their own lines.
- Names declared together, function results, array element types, aliases,
  subtypes and attribute specifications have a break point as well.

A blank line between declarations or statements is kept (at most one), but
not at the start of a block or directly above `begin`, `else` or `end`. A
comment stays on the line it was written on. Processes and subprogram bodies
are set apart by a blank line, as is a design unit from its context clause.

Spacing inside a line and trailing whitespace are normalized. Reserved words
follow `keyword_case`, and so do the names of predefined attributes such as
`'length` and `'event`. Declarations are written `name: type`, or `name : type`
with `space_before_colon`; the colon of a label never has a space before it.

Columns can be aligned on request. Rows are aligned with their neighbours; a
blank line, a comment line or a row that does not fit on one line ends the run.

- `align_declarations`: the types of object, record element and interface
  declarations, their `:=`, and the types after port modes. With
  `space_before_colon` the colons are lined up as well.
- `align_associations`: the arrows of generic and port maps and of expanded
  named aggregates.
- `align_assignments`: the operators of unlabeled assignments.
- `align_comments`: the comments that end neighbouring lines of the same
  indentation, unless that would push a line over the width.

With `align_declarations` or `align_associations` on, a blank line between the
entries of the aligned list is kept and separates the runs.

Text without a break point, such as a long string or a signature, may exceed
the width. Layouts that are not ported from `vhdl_lang` yet are listed as
ignored tests in `tests/layout_*.rs`.

`vhdl_syntax` parses VHDL-2008. Files that need VHDL-2019 syntax, such as mode
views, are rejected with a syntax error.

## Usage

```sh
vhdl-fmt                 # format every .vhd/.vhdl file below the working directory
vhdl-fmt rtl/ tb/top.vhd # format directories and files in place
vhdl-fmt --check rtl/    # list files that would change; exit 1 if any
vhdl-fmt --diff rtl/     # print the changes as a diff; exit 1 if any
vhdl-fmt - < design.vhd  # format standard input to standard output
```

Directory searches skip hidden files and anything git ignores; `--exclude GLOB`
skips more. A file that fails to parse is reported as `path:line:column:
message` and does not stop the other files. The exit code is 2 if any file
failed.

Each file is configured by the `[format]` table and `standard` key of the
nearest `vhdl_ls.toml` in its ancestors. `--config FILE` names a project file
explicitly, `--isolated` ignores project files, and `--max-width`,
`--indent-width`, `--keyword-case`, `--standard`, `--space-before-colon` and
the `--align-*` flags override them.

```toml
standard = "2008"

[format]
max_width = 100
indent_width = 4
keyword_case = "lower"    # or "upper"
inline_argument_limit = 2 # entries a list may have and stay on one line
align_declarations = false
align_associations = false
align_assignments = false
align_comments = false
space_before_colon = false
```

Source between `-- fmt: off` and `-- fmt: on` is copied verbatim. `-- fmt: skip`
on the line above a declaration or statement, or at the end of its last line,
does the same for that one item.

## Editor integration

Any editor that can pipe a buffer through a command can use `vhdl-fmt`: pass
`-` to read standard input and write standard output, and the file's path as
`--stdin-filename` so that the nearest `vhdl_ls.toml` is found. If the buffer
does not parse, the command fails and prints nothing to standard output.

### Zed

Install the VHDL extension, then add this to `settings.json`:

```json
"languages": {
  "VHDL": {
    "format_on_save": "on",
    "formatter": {
      "external": {
        "command": "vhdl-fmt",
        "arguments": ["-", "--stdin-filename", "{buffer_path}"]
      }
    }
  }
}
```

`command` must be on the `PATH` (`cargo install --path vhdl-fmt`) or an
absolute path such as `/path/to/rust_hdl/target/release/vhdl-fmt`. Style
options can be appended to `arguments`, but a `[format]` table in the project
file is shared with the command line and continuous integration. When a file
cannot be formatted, Zed leaves the buffer unchanged and writes the reason to
its log (`zed: open log`).

## Safety

The output is parsed again and must hold the same tokens, with the same
comments before each, as the input. If it does not, nothing is written and the
difference is reported with its location. Input is handled as bytes, so Latin-1
and UTF-8 files are both written back in their original encoding.

## Testing

`cargo test -p vhdl-fmt` formats the VHDL in this repository. To include other
source trees, list them in `VHDL_FMT_CORPUS`:

```sh
VHDL_FMT_CORPUS=~/src/nvc/test cargo test -p vhdl-fmt --test corpus
```

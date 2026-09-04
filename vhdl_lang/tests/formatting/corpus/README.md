# Bundled formatter corpus

These six IEEE library sources are byte-for-byte snapshots of
`vhdl_libraries/ieee2008` at repository commit
`0b10062948ba24c542c53cbd1c0bd2b2b92706f0`, the last change to that directory
before the formatter work. Their original copyright and Apache-2.0 license
notices are retained in each file; [LICENSE](LICENSE) contains the Apache-2.0
license text. Do not format these inputs: they represent
real-world source layout, not golden formatter output.

The snapshots live inside `vhdl_lang` so Cargo source packages include all data
required by the formatter corpus tests and benchmark. When updating them, copy
the corresponding upstream files unchanged, record the source commit here,
and run `cargo test -p vhdl_lang --test format_corpus`.

SHA-256 checksums:

```text
33fe4fe3fc21cbe6c36ed4969d96ed25549680bb3d936f106078fe47af2fec7b  ieee2008/math_real.vhdl
ed057e95cd908b547d128d6a29dbfcf243ba64468d6e6cc780090bc9cd79f3b2  ieee2008/math_real-body.vhdl
2a34c7d7b2c8ba21b1e91153741399cf2cd23c8b04028dcf53765efeea76de55  ieee2008/std_logic_1164.vhdl
6534fe4842c1133199db93725e36a9e973ea8e2ab03890433c013af813d5ce2c  ieee2008/std_logic_1164-body.vhdl
c72dea068fddc9f07f0bf8165aa7394c37d19e283e10dfc6817fdda0f2bcbe78  ieee2008/numeric_std.vhdl
10e8bdc4fedc881a972f5900abe833d24397d686e07b566479c47495acf39721  ieee2008/numeric_std-body.vhdl
```

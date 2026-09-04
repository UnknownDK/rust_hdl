use brunch::{Bench, Benches};
use std::{hint::black_box, path::Path};
use vhdl_lang::{format_source, Source, VHDLParser, VHDLStandard};

fn main() {
    let mut benches = Benches::default();
    for (name, input) in [
        (
            "formatter medium: std_logic_1164 body",
            include_str!("../../vhdl_libraries/ieee2008/std_logic_1164-body.vhdl"),
        ),
        (
            "formatter large: numeric_std body",
            include_str!("../../vhdl_libraries/ieee2008/numeric_std-body.vhdl"),
        ),
    ] {
        let source = Source::inline(Path::new("benchmark.vhd"), input);
        let parser = VHDLParser::new(VHDLStandard::VHDL2008);
        // Includes both parsing passes and all preservation checks: the same
        // work performed by the format-on-save API.
        benches.push(Bench::new(name).with_samples(30).run(|| {
            black_box(format_source(&parser, &source).unwrap());
        }));
    }
    benches.finish();
}

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this file,
// You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2018, Olof Kraigher olof.kraigher@gmail.com

use clap::Parser;
use itertools::Itertools;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use vhdl_lang::{
    format_source, Config, Diagnostic, FormatError, MessagePrinter, Project, Severity, SeverityMap,
    Source, VHDLParser, VHDLStandard,
};

#[derive(Debug, clap::Args)]
#[group(required = true, multiple = false)]
pub struct Group {
    /// Config file in TOML format containing libraries and settings
    #[arg(short, long)]
    config: Option<String>,

    /// Format the passed file and write the contents to stdout.
    ///
    /// This is experimental and the formatting behavior will change in the future.
    #[arg(short, long)]
    format: Option<PathBuf>,

    /// Format VHDL read from stdin and write the complete result to stdout.
    #[arg(long)]
    format_stdin: bool,
}

/// Run vhdl analysis
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The number of threads to use. By default, the maximum is selected based on process cores
    #[arg(short = 'p', long)]
    num_threads: Option<usize>,

    /// Path to the config file for the VHDL standard libraries (i.e., IEEE std_logic_1164).
    /// If omitted, will search for these libraries in a set of standard paths
    #[arg(short = 'l', long)]
    libraries: Option<String>,

    /// Optional source path metadata for diagnostics in --format-stdin mode.
    #[arg(long, requires = "format_stdin")]
    stdin_filepath: Option<PathBuf>,

    #[clap(flatten)]
    group: Group,
}

fn main() {
    let args = Args::parse();
    if let Some(config_path) = args.group.config {
        parse_and_analyze_project(&config_path, args.num_threads, args.libraries.as_ref());
    } else if let Some(format) = args.group.format {
        run_formatter(format_file(&format));
    } else if args.group.format_stdin {
        run_formatter(format_stdin(args.stdin_filepath.as_deref()));
    }
}

fn run_formatter(result: Result<(), CliFormatError>) {
    if let Err(err) = result {
        show_format_error(&err);
        std::process::exit(if matches!(err, CliFormatError::Format(_)) {
            1
        } else {
            2
        });
    }
}

fn format_file(path: &Path) -> Result<(), CliFormatError> {
    let parser = VHDLParser::new(VHDLStandard::default());
    let source = Source::from_latin1_file(path)?;
    write_stdout(&format_source(&parser, &source)?)
}

fn format_stdin(path: Option<&Path>) -> Result<(), CliFormatError> {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;
    let source = Source::inline(path.unwrap_or_else(|| Path::new("<stdin>.vhd")), &input);
    let parser = VHDLParser::new(VHDLStandard::default());
    write_stdout(&format_source(&parser, &source)?)
}

fn write_stdout(output: &str) -> Result<(), CliFormatError> {
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    stdout.write_all(output.as_bytes())?;
    stdout.flush()?;
    Ok(())
}

#[derive(Debug)]
enum CliFormatError {
    Io(io::Error),
    Format(FormatError),
}

impl From<io::Error> for CliFormatError {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<FormatError> for CliFormatError {
    fn from(err: FormatError) -> Self {
        Self::Format(err)
    }
}

fn show_format_error(err: &CliFormatError) {
    match err {
        CliFormatError::Io(err) => eprintln!("{err}"),
        CliFormatError::Format(FormatError::InputDiagnostics(diagnostics)) => {
            eprintln!("input contains {} parse diagnostic(s)", diagnostics.len());
            show_diagnostics_to(diagnostics, &SeverityMap::default(), &mut io::stderr());
        }
        CliFormatError::Format(FormatError::OutputDiagnostics(diagnostics)) => {
            eprintln!(
                "formatted output contains {} parse diagnostic(s)",
                diagnostics.len()
            );
            show_diagnostics_to(diagnostics, &SeverityMap::default(), &mut io::stderr());
        }
        CliFormatError::Format(err) => eprintln!("{err}"),
    }
}

fn parse_and_analyze_project(
    config_path: &str,
    num_threads: Option<usize>,
    libraries: Option<&String>,
) {
    rayon::ThreadPoolBuilder::new()
        .num_threads(num_threads.unwrap_or(0))
        .build_global()
        .unwrap();

    let mut config = Config::default();
    let mut msg_printer = MessagePrinter::default();
    config.load_external_config(&mut msg_printer, libraries.cloned());
    let project_config = match Config::read_file_path(Path::new(&config_path)) {
        Ok(project_config) => project_config,
        Err(err) => {
            eprintln!("error: Failed to read config file '{config_path}': {err}");
            std::process::exit(2);
        }
    };
    config.append(&project_config, &mut msg_printer);

    let severity_map = *config.severities();
    let mut project = Project::from_config(config, &mut msg_printer);
    project.enable_all_linters();
    let diagnostics = project.analyse();

    show_diagnostics(&diagnostics, &severity_map);

    if diagnostics
        .iter()
        .any(|diag| severity_map[diag.code].is_some_and(|severity| severity == Severity::Error))
    {
        std::process::exit(1);
    } else {
        std::process::exit(0);
    }
}

/// Analysis runs in parallel, so diagnostics arrive in a different order on every run.
/// Sort them by file, then position, then code and message, so the printed output is the
/// same from run to run and can be compared or diffed.
fn in_output_order(diagnostics: &[Diagnostic]) -> Vec<&Diagnostic> {
    let mut sorted: Vec<&Diagnostic> = diagnostics.iter().collect();
    sorted.sort_by(|a, b| {
        a.pos
            .cmp(&b.pos)
            .then_with(|| <&'static str>::from(a.code).cmp(<&'static str>::from(b.code)))
            .then_with(|| a.message.cmp(&b.message))
    });
    sorted
}

fn show_diagnostics(diagnostics: &[Diagnostic], severity_map: &SeverityMap) {
    show_diagnostics_to(diagnostics, severity_map, &mut io::stdout());
}

fn show_diagnostics_to(
    diagnostics: &[Diagnostic],
    severity_map: &SeverityMap,
    writer: &mut dyn Write,
) {
    let diagnostics = in_output_order(diagnostics)
        .into_iter()
        .filter_map(|diag| diag.show(severity_map))
        .collect_vec();
    for str in &diagnostics {
        let _ = writeln!(writer, "{str}");
    }

    if !diagnostics.is_empty() {
        let _ = writeln!(writer, "Found {} diagnostics", diagnostics.len());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_errors(file_name: &str, contents: &str) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        VHDLParser::new(VHDLStandard::default()).parse_design_source(
            &Source::inline(Path::new(file_name), contents),
            &mut diagnostics,
        );
        diagnostics
    }

    #[test]
    fn diagnostics_are_printed_by_file_then_position() {
        // One syntax error on each line, in two files.
        let mut diagnostics = parse_errors("b.vhd", "entity x is end entity y;\n");
        diagnostics.extend(parse_errors(
            "a.vhd",
            "entity x is end entity y;\nentity z is end entity w;\n",
        ));
        assert_eq!(diagnostics.len(), 3);
        // Reverse the order the analysis produced, as a parallel run may.
        diagnostics.reverse();
        let places: Vec<(String, u32)> = in_output_order(&diagnostics)
            .iter()
            .map(|d| {
                (
                    d.pos
                        .file_name()
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .into_owned(),
                    d.pos.range.start.line,
                )
            })
            .collect();
        assert_eq!(
            places,
            vec![
                ("a.vhd".to_string(), 0),
                ("a.vhd".to_string(), 1),
                ("b.vhd".to_string(), 0)
            ]
        );
    }
}

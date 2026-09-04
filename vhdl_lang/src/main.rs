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
    format_text_with_config, Config, Diagnostic, FormatConfig, FormatError, KeywordCase,
    Latin1String, MessagePrinter, Project, Severity, SeverityMap, VHDLParser, VHDLStandard,
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

    /// Source path for diagnostics and project configuration discovery in stdin mode.
    #[arg(long, requires = "format_stdin")]
    stdin_filepath: Option<PathBuf>,

    /// Preferred formatter line width (unbreakable text may exceed this).
    #[arg(long)]
    max_width: Option<usize>,

    /// Spaces per formatter indentation level.
    #[arg(long)]
    indent_width: Option<usize>,

    /// Case of reserved words; identifiers, literals and comments are unchanged.
    #[arg(long, value_enum)]
    keyword_case: Option<KeywordCase>,

    /// Align declaration colons (pass false to override project settings).
    #[arg(long, num_args = 0..=1, default_missing_value = "true", require_equals = true)]
    align_declarations: Option<bool>,

    /// Align named port/generic map arrows (pass false to override project settings).
    #[arg(long, num_args = 0..=1, default_missing_value = "true", require_equals = true)]
    align_associations: Option<bool>,

    /// Explicit formatter project TOML file, instead of ancestor discovery.
    #[arg(long, conflicts_with = "no_format_config")]
    format_config: Option<PathBuf>,

    /// Ignore project formatter configuration and use defaults plus CLI overrides.
    #[arg(long)]
    no_format_config: bool,

    /// VHDL standard used by the formatter (default: 2008).
    #[arg(long, value_parser = ["1993", "2008", "2019"])]
    standard: Option<String>,

    #[clap(flatten)]
    group: Group,
}

fn main() {
    let args = Args::parse();
    if let Some(config_path) = &args.group.config {
        parse_and_analyze_project(config_path, args.num_threads, args.libraries.as_ref());
    } else {
        run_formatter((|| {
            let (config, standard) = formatter_settings(&args)?;
            if let Some(path) = &args.group.format {
                format_file(path, &config, standard)
            } else {
                format_stdin(args.stdin_filepath.as_deref(), &config, standard)
            }
        })());
    }
}

fn formatter_settings(args: &Args) -> Result<(FormatConfig, VHDLStandard), CliFormatError> {
    let path = if args.no_format_config {
        None
    } else if let Some(path) = &args.format_config {
        Some(path.clone())
    } else {
        let cwd = std::env::current_dir()?;
        let source = args.group.format.as_ref().or(args.stdin_filepath.as_ref());
        // Normalize lexical `..` even for buffers whose file does not exist yet.
        let source = source.map(|source| {
            let mut path = PathBuf::new();
            for component in cwd.join(source).components() {
                match component {
                    std::path::Component::ParentDir => {
                        path.pop();
                    }
                    std::path::Component::CurDir => {}
                    component => path.push(component.as_os_str()),
                }
            }
            path
        });
        let directory = source.as_deref().and_then(Path::parent).unwrap_or(&cwd);
        let mut found = None;
        for ancestor in directory.ancestors() {
            let candidate = ancestor.join("vhdl_ls.toml");
            match std::fs::metadata(&candidate) {
                Ok(_) => {
                    found = Some(candidate);
                    break;
                }
                Err(err) if err.kind() == io::ErrorKind::NotFound => {}
                Err(err) => {
                    return Err(CliFormatError::Config(format!(
                        "{}: {err}",
                        candidate.display()
                    )))
                }
            }
        }
        found
    };
    let mut config = FormatConfig::default();
    let mut standard = VHDLStandard::default();
    if let Some(path) = path {
        let load = || -> Result<(FormatConfig, VHDLStandard), String> {
            let text = std::fs::read_to_string(&path).map_err(|err| err.to_string())?;
            let config = FormatConfig::from_toml(&text)?;
            let root = text.parse::<toml::Table>().map_err(|err| err.to_string())?;
            let standard = if let Some(value) = root.get("standard") {
                VHDLStandard::try_from(value.as_str().ok_or("standard must be a string")?)
                    .map_err(|()| "standard must be 1993, 2008 or 2019")?
            } else {
                VHDLStandard::default()
            };
            Ok((config, standard))
        };
        (config, standard) =
            load().map_err(|err| CliFormatError::Config(format!("{}: {err}", path.display())))?;
    }
    if let Some(width) = args.max_width {
        config.max_width = width;
    }
    if let Some(width) = args.indent_width {
        config.indent_width = width;
    }
    if let Some(case) = args.keyword_case {
        config.keyword_case = case;
    }
    if let Some(align) = args.align_declarations {
        config.align_declarations = align;
    }
    if let Some(align) = args.align_associations {
        config.align_associations = align;
    }
    if let Some(value) = &args.standard {
        standard = VHDLStandard::try_from(value.as_str()).expect("clap validates the standard");
    }
    config.validate().map_err(CliFormatError::Config)?;
    Ok((config, standard))
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

fn format_file(
    path: &Path,
    config: &FormatConfig,
    standard: VHDLStandard,
) -> Result<(), CliFormatError> {
    let parser = VHDLParser::new(standard);
    let input = Latin1String::from_vec(std::fs::read(path)?).to_string();
    write_stdout(&format_text_with_config(&parser, path, &input, config)?)
}

fn format_stdin(
    path: Option<&Path>,
    config: &FormatConfig,
    standard: VHDLStandard,
) -> Result<(), CliFormatError> {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;
    let parser = VHDLParser::new(standard);
    write_stdout(&format_text_with_config(
        &parser,
        path.unwrap_or_else(|| Path::new("<stdin>.vhd")),
        &input,
        config,
    )?)
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
    Config(String),
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
        CliFormatError::Config(message) => eprintln!("formatter configuration: {message}"),
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
    config.append(
        &Config::read_file_path(Path::new(&config_path)).expect("Failed to read config file"),
        &mut msg_printer,
    );

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

fn show_diagnostics(diagnostics: &[Diagnostic], severity_map: &SeverityMap) {
    show_diagnostics_to(diagnostics, severity_map, &mut io::stdout());
}

fn show_diagnostics_to(
    diagnostics: &[Diagnostic],
    severity_map: &SeverityMap,
    writer: &mut dyn Write,
) {
    let diagnostics = diagnostics
        .iter()
        .filter_map(|diag| diag.show(severity_map))
        .collect_vec();
    for str in &diagnostics {
        let _ = writeln!(writer, "{str}");
    }

    if !diagnostics.is_empty() {
        let _ = writeln!(writer, "Found {} diagnostics", diagnostics.len());
    }
}

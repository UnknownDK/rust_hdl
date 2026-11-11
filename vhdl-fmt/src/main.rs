// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this file,
// You can obtain one at http://mozilla.org/MPL/2.0/.

use clap::Parser;
use ignore::{overrides::OverrideBuilder, WalkBuilder};
use rayon::prelude::*;
use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use vhdl_fmt::{format, FormatError, KeywordCase, Settings, CONFIG_NAME};
use vhdl_syntax::standard::VHDLStandard;

/// Format VHDL files in place.
#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    /// Files to format, or directories to search for .vhd and .vhdl files.
    /// `-` formats standard input to standard output.
    #[arg(default_value = ".")]
    paths: Vec<PathBuf>,

    /// Write nothing; list the files that would change and exit with 1 if any.
    #[arg(long)]
    check: bool,

    /// Write nothing; print the changes as a diff and exit with 1 if any.
    #[arg(long)]
    diff: bool,

    /// Path of the standard input's file, used to find its configuration and
    /// to name it in messages.
    #[arg(long, value_name = "PATH")]
    stdin_filename: Option<PathBuf>,

    /// Use this project file instead of the nearest `vhdl_ls.toml`.
    #[arg(long, value_name = "FILE", conflicts_with = "isolated")]
    config: Option<PathBuf>,

    /// Ignore all project files.
    #[arg(long)]
    isolated: bool,

    /// Exclude paths matching this glob, relative to the working directory.
    /// May be repeated.
    #[arg(long, value_name = "GLOB")]
    exclude: Vec<String>,

    /// Case of reserved words.
    #[arg(long, value_enum)]
    keyword_case: Option<KeywordCase>,

    /// Align the colons of adjacent declarations.
    #[arg(long)]
    align_declarations: bool,

    /// Align the arrows of adjacent named associations.
    #[arg(long)]
    align_associations: bool,

    /// Align the operators of adjacent assignments.
    #[arg(long)]
    align_assignments: bool,

    /// Line up the comments that end neighbouring lines of code.
    #[arg(long)]
    align_comments: bool,

    /// Write `name : type` in declarations, not `name: type`.
    #[arg(long)]
    space_before_colon: bool,

    /// Preferred line width.
    #[arg(long, value_name = "COLUMNS")]
    max_width: Option<usize>,

    /// Spaces per indentation level.
    #[arg(long, value_name = "SPACES")]
    indent_width: Option<usize>,

    /// VHDL revision, for example 2008.
    #[arg(long, value_parser = parse_standard)]
    standard: Option<VHDLStandard>,
}

fn parse_standard(value: &str) -> Result<VHDLStandard, String> {
    value
        .parse()
        .map_err(|_| "expected a VHDL revision such as 1993, 2008 or 2019".to_owned())
}

const EXIT_CHANGES: u8 = 1;
const EXIT_ERROR: u8 = 2;

/// Finds the settings that apply to a file: the nearest project file in its
/// ancestors, then command-line overrides.
struct Resolver<'a> {
    args: &'a Args,
    by_directory: HashMap<PathBuf, Result<Settings, String>>,
}

impl Resolver<'_> {
    fn load(path: &Path) -> Result<Settings, String> {
        std::fs::read_to_string(path)
            .map_err(|err| err.to_string())
            .and_then(|text| Settings::from_toml(&text))
            .map_err(|err| format!("{}: {err}", path.display()))
    }

    fn project(&mut self, file: &Path) -> Result<Settings, String> {
        if self.args.isolated {
            return Ok(Settings::default());
        }
        if let Some(config) = &self.args.config {
            return Self::load(config);
        }
        let file = std::path::absolute(file).map_err(|err| err.to_string())?;
        let Some(directory) = file.parent() else {
            return Ok(Settings::default());
        };
        if let Some(settings) = self.by_directory.get(directory) {
            return settings.clone();
        }
        let settings = directory
            .ancestors()
            .map(|ancestor| ancestor.join(CONFIG_NAME))
            .find(|candidate| candidate.is_file())
            .map_or_else(|| Ok(Settings::default()), |path| Self::load(&path));
        self.by_directory
            .insert(directory.to_owned(), settings.clone());
        settings
    }

    fn resolve(&mut self, file: &Path) -> Result<Settings, String> {
        let mut settings = self.project(file)?;
        if let Some(case) = self.args.keyword_case {
            settings.format.keyword_case = case;
        }
        settings.format.align_declarations |= self.args.align_declarations;
        settings.format.align_associations |= self.args.align_associations;
        settings.format.align_assignments |= self.args.align_assignments;
        settings.format.align_comments |= self.args.align_comments;
        settings.format.space_before_colon |= self.args.space_before_colon;
        if let Some(width) = self.args.max_width {
            settings.format.max_width = width;
        }
        if let Some(width) = self.args.indent_width {
            settings.format.indent_width = width;
        }
        if let Some(standard) = self.args.standard {
            settings.standard = standard;
        }
        Ok(settings)
    }
}

/// Explicitly named files are always selected. Directories are searched for
/// VHDL files, skipping hidden files and anything ignored by git.
fn discover(args: &Args) -> Result<(Vec<PathBuf>, Vec<String>), String> {
    let mut overrides = OverrideBuilder::new(std::env::current_dir().map_err(|e| e.to_string())?);
    for pattern in &args.exclude {
        overrides
            .add(&format!("!{pattern}"))
            .map_err(|err| format!("invalid --exclude pattern: {err}"))?;
    }
    let mut builder = WalkBuilder::new(&args.paths[0]);
    for path in &args.paths[1..] {
        builder.add(path);
    }
    builder.overrides(overrides.build().map_err(|err| err.to_string())?);
    let mut files = Vec::new();
    let mut errors = Vec::new();
    for entry in builder.build() {
        match entry {
            Ok(entry) => {
                let is_vhdl = entry
                    .path()
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .is_some_and(|extension| {
                        extension.eq_ignore_ascii_case("vhd")
                            || extension.eq_ignore_ascii_case("vhdl")
                    });
                if entry.file_type().is_some_and(|kind| kind.is_file())
                    && (entry.depth() == 0 || is_vhdl)
                {
                    files.push(entry.into_path());
                }
            }
            Err(err) => errors.push(err.to_string()),
        }
    }
    files.sort();
    files.dedup();
    Ok((files, errors))
}

enum Outcome {
    Unchanged,
    Changed { input: Vec<u8>, output: Vec<u8> },
    Failed(String),
}

fn write_atomically(path: &Path, contents: &[u8]) -> io::Result<()> {
    // Replace the file a symbolic link points to, not the link.
    let path = std::fs::canonicalize(path)?;
    let directory = path.parent().unwrap_or(Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(directory)?;
    temporary.write_all(contents)?;
    temporary
        .as_file()
        .set_permissions(std::fs::metadata(&path)?.permissions())?;
    temporary.as_file().sync_all()?;
    temporary.persist(&path).map_err(|err| err.error)?;
    Ok(())
}

fn format_file(path: &Path, settings: Result<Settings, String>, write: bool) -> Outcome {
    let run = || -> Result<Outcome, String> {
        let settings = settings?;
        let input = std::fs::read(path).map_err(|err| format!("{}: {err}", path.display()))?;
        let output = format(&input, &settings).map_err(|err| located(path, &err))?;
        if input == output {
            return Ok(Outcome::Unchanged);
        }
        if write {
            write_atomically(path, &output).map_err(|err| format!("{}: {err}", path.display()))?;
        }
        Ok(Outcome::Changed { input, output })
    };
    run().unwrap_or_else(Outcome::Failed)
}

/// One `path:line:column: message` line per problem. A single mistake often
/// produces a cascade of syntax errors, so only the first few are shown.
fn located(path: &Path, error: &FormatError) -> String {
    const SHOWN: usize = 3;
    if matches!(error, FormatError::InvalidConfig(_)) {
        return error.to_string();
    }
    let text = error.to_string();
    let mut lines: Vec<_> = text
        .lines()
        .take(SHOWN)
        .map(|line| format!("{}:{line}", path.display()))
        .collect();
    let hidden = text.lines().count().saturating_sub(SHOWN);
    if hidden > 0 {
        lines.push(format!(
            "{}: {hidden} more syntax error{}",
            path.display(),
            if hidden == 1 { "" } else { "s" }
        ));
    }
    lines.join("\n")
}

fn diff(path: &Path, input: &[u8], output: &[u8]) -> String {
    let (input, output) = (
        String::from_utf8_lossy(input),
        String::from_utf8_lossy(output),
    );
    let label = path.display().to_string();
    similar::TextDiff::from_lines(input.as_ref(), output.as_ref())
        .unified_diff()
        .context_radius(3)
        .header(&label, &label)
        .to_string()
}

fn plural(count: usize) -> String {
    format!("{count} file{}", if count == 1 { "" } else { "s" })
}

fn stdin(args: &Args) -> Result<u8, String> {
    let name = args
        .stdin_filename
        .clone()
        .unwrap_or_else(|| PathBuf::from("<stdin>"));
    let mut resolver = Resolver {
        args,
        by_directory: HashMap::new(),
    };
    // Without a file name, the working directory decides the project file.
    let settings = resolver.resolve(args.stdin_filename.as_deref().unwrap_or(Path::new("-")))?;
    let mut input = Vec::new();
    io::stdin()
        .read_to_end(&mut input)
        .map_err(|err| err.to_string())?;
    let output = format(&input, &settings).map_err(|err| located(&name, &err))?;
    let changed = input != output;
    let mut stdout = io::stdout().lock();
    let written = if args.diff {
        stdout.write_all(diff(&name, &input, &output).as_bytes())
    } else if args.check {
        if changed {
            writeln!(stdout, "Would reformat: {}", name.display())
        } else {
            Ok(())
        }
    } else {
        stdout.write_all(&output)
    };
    written
        .and_then(|()| stdout.flush())
        .map_err(|err| err.to_string())?;
    Ok(if changed && (args.check || args.diff) {
        EXIT_CHANGES
    } else {
        0
    })
}

fn files(args: &Args) -> Result<u8, String> {
    let (paths, mut errors) = discover(args)?;
    let mut resolver = Resolver {
        args,
        by_directory: HashMap::new(),
    };
    let jobs: Vec<_> = paths
        .into_iter()
        .map(|path| {
            let settings = resolver.resolve(&path);
            (path, settings)
        })
        .collect();
    let write = !args.check && !args.diff;
    // Every file is attempted; one that fails does not stop the others.
    let outcomes: Vec<_> = jobs
        .into_par_iter()
        .map(|(path, settings)| {
            let outcome = format_file(&path, settings, write);
            (path, outcome)
        })
        .collect();

    let mut stdout = io::stdout().lock();
    let (mut changed, mut unchanged, mut failed) = (0, 0, 0);
    let mut print = |text: &str| stdout.write_all(text.as_bytes());
    for (path, outcome) in &outcomes {
        let printed = match outcome {
            Outcome::Unchanged => {
                unchanged += 1;
                Ok(())
            }
            Outcome::Changed { input, output } => {
                changed += 1;
                if args.diff {
                    print(&diff(path, input, output))
                } else if args.check {
                    print(&format!("Would reformat: {}\n", path.display()))
                } else {
                    Ok(())
                }
            }
            Outcome::Failed(message) => {
                failed += 1;
                errors.extend(message.lines().map(str::to_owned));
                Ok(())
            }
        };
        printed.map_err(|err| err.to_string())?;
    }
    for error in &errors {
        eprintln!("error: {error}");
    }
    let summary = if write {
        format!(
            "{} reformatted, {} left unchanged",
            plural(changed),
            plural(unchanged)
        )
    } else {
        format!(
            "{} would be reformatted, {} already formatted",
            plural(changed),
            plural(unchanged)
        )
    };
    let summary = if failed == 0 {
        summary
    } else {
        format!("{summary}, {} failed", plural(failed))
    };
    // A diff on standard output stays a valid patch.
    if args.diff {
        eprintln!("{summary}");
    } else {
        print(&format!("{summary}\n")).map_err(|err| err.to_string())?;
    }
    Ok(if !errors.is_empty() {
        EXIT_ERROR
    } else if changed > 0 && !write {
        EXIT_CHANGES
    } else {
        0
    })
}

fn main() -> ExitCode {
    let args = Args::parse();
    let from_stdin = args.paths == [PathBuf::from("-")];
    let result = if from_stdin {
        stdin(&args)
    } else if args.stdin_filename.is_some() {
        Err("--stdin-filename requires `-` as the only path".to_owned())
    } else {
        files(&args)
    };
    match result {
        Ok(code) => ExitCode::from(code),
        Err(message) => {
            for line in message.lines() {
                eprintln!("error: {line}");
            }
            ExitCode::from(EXIT_ERROR)
        }
    }
}

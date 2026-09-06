// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this file,
// You can obtain one at http://mozilla.org/MPL/2.0/.

use super::*;
use std::collections::BTreeMap;
use std::fs;
use tempfile::{NamedTempFile, TempPath};

struct FormattedFile {
    path: PathBuf,
    original: Vec<u8>,
    input: String,
    output: String,
    bytes: Vec<u8>,
}

fn error(path: &Path, message: impl std::fmt::Display) -> CliFormatError {
    CliFormatError::Io(io::Error::other(format!("{}: {message}", path.display())))
}

fn excluded(path: &Path, roots: &[PathBuf], patterns: &[glob::Pattern]) -> bool {
    patterns.iter().any(|pattern| {
        pattern.matches_path(path)
            || path
                .file_name()
                .is_some_and(|name| pattern.matches_path(Path::new(name)))
            || roots.iter().any(|root| {
                path.strip_prefix(root)
                    .is_ok_and(|relative| pattern.matches_path(relative))
            })
    })
}

fn discover(
    path: &Path,
    explicit: bool,
    roots: &[PathBuf],
    patterns: &[glob::Pattern],
    files: &mut BTreeMap<PathBuf, PathBuf>,
) -> Result<(), CliFormatError> {
    if excluded(path, roots, patterns) {
        return Ok(());
    }
    let metadata = fs::symlink_metadata(path).map_err(|err| error(path, err))?;
    if metadata.file_type().is_symlink() {
        return if explicit {
            Err(error(
                path,
                "symbolic links are not followed; select the real file instead",
            ))
        } else {
            Ok(())
        };
    }
    if metadata.is_dir() {
        if path.file_name().is_some_and(|name| {
            [".git", ".hg", ".svn"]
                .iter()
                .any(|excluded| name == *excluded)
        }) {
            return Ok(());
        }
        let mut entries = fs::read_dir(path)
            .map_err(|err| error(path, err))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| error(path, err))?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            discover(&entry.path(), false, roots, patterns, files)?;
        }
    } else if metadata.is_file() {
        if explicit
            || path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| {
                    extension.eq_ignore_ascii_case("vhd") || extension.eq_ignore_ascii_case("vhdl")
                })
        {
            let canonical = fs::canonicalize(path).map_err(|err| error(path, err))?;
            files.entry(canonical).or_insert_with(|| path.to_owned());
        }
    } else if explicit {
        return Err(error(path, "expected a regular file or directory"));
    }
    Ok(())
}

fn encode(text: &str, encoding: InputEncoding) -> Result<Vec<u8>, CliFormatError> {
    match encoding {
        InputEncoding::Utf8 => Ok(text.as_bytes().to_vec()),
        InputEncoding::Latin1 => text
            .chars()
            .map(|ch| {
                u8::try_from(ch as u32).map_err(|_| {
                    CliFormatError::Io(io::Error::other(
                        "formatted output is not representable in Latin-1",
                    ))
                })
            })
            .collect(),
    }
}

fn report(input: &str, output: &str, path: &Path, args: &Args) -> Result<(), CliFormatError> {
    if input == output {
        return Ok(());
    }
    if args.diff {
        let label = path.to_string_lossy();
        let diff = similar::TextDiff::from_lines(input, output);
        write_stdout(
            &diff
                .unified_diff()
                .context_radius(3)
                .header(&format!("a/{label}"), &format!("b/{label}"))
                .to_string(),
        )?;
    } else if args.check {
        writeln!(io::stdout().lock(), "Would reformat {}", path.display())?;
    }
    Ok(())
}

pub(super) fn run(args: &Args) -> Result<i32, CliFormatError> {
    if args.group.format_stdin {
        let path = args
            .stdin_filepath
            .as_deref()
            .unwrap_or_else(|| Path::new("<stdin>.vhd"));
        let (config, standard) = formatter_settings(args, args.stdin_filepath.as_deref())?;
        let mut bytes = Vec::new();
        io::stdin().read_to_end(&mut bytes)?;
        let input = args.input_encoding.decode(bytes)?;
        let output = format_text_with_config(&VHDLParser::new(standard), path, &input, &config)?;
        if args.check || args.diff {
            report(&input, &output, path, args)?;
            return Ok(i32::from(input != output));
        }
        write_stdout(&output)?;
        return Ok(0);
    }
    let patterns = args
        .exclude
        .iter()
        .map(|pattern| {
            glob::Pattern::new(pattern).map_err(|err| {
                CliFormatError::Config(format!("invalid exclude pattern {pattern:?}: {err}"))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let cwd = std::env::current_dir()?;
    let mut roots = vec![cwd];
    roots.extend(
        args.group
            .format
            .iter()
            .filter(|path| path.is_dir())
            .cloned(),
    );
    let mut paths = BTreeMap::new();
    for path in &args.group.format {
        discover(path, true, &roots, &patterns, &mut paths)?;
    }
    if !args.write
        && !args.check
        && !args.diff
        && (args.group.format.len() != 1 || args.group.format[0].is_dir())
    {
        return Err(CliFormatError::Config(
            "multiple files or directories require --write, --check, or --diff".into(),
        ));
    }
    let mut paths: Vec<_> = paths.into_values().collect();
    paths.sort();
    let mut files = Vec::new();
    // Prepare every result before producing diffs or replacing any input.
    for path in paths {
        let (config, standard) = formatter_settings(args, Some(&path))?;
        let original = fs::read(&path).map_err(|err| error(&path, err))?;
        let input = args
            .input_encoding
            .decode(original.clone())
            .map_err(|err| error(&path, err))?;
        let output = format_text_with_config(&VHDLParser::new(standard), &path, &input, &config)?;
        let bytes = encode(&output, args.input_encoding)?;
        files.push(FormattedFile {
            path,
            original,
            input,
            output,
            bytes,
        });
    }
    let changed = files.iter().any(|file| file.original != file.bytes);
    if args.write {
        write_files(&files)?;
    } else if args.check || args.diff {
        for file in &files {
            report(&file.input, &file.output, &file.path, args)?;
        }
    } else if let Some(file) = files.first() {
        write_stdout(&file.output)?;
    }
    Ok(if args.check || args.diff {
        i32::from(changed)
    } else {
        0
    })
}

fn writable(file: &FormattedFile) -> Result<fs::Permissions, CliFormatError> {
    let metadata = fs::symlink_metadata(&file.path).map_err(|err| error(&file.path, err))?;
    if !metadata.is_file() || metadata.permissions().readonly() {
        return Err(error(
            &file.path,
            "refusing to replace a non-regular or read-only file",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.nlink() != 1 {
            return Err(error(&file.path, "refusing to replace a hard-linked file"));
        }
    }
    if fs::read(&file.path).map_err(|err| error(&file.path, err))? != file.original {
        return Err(error(
            &file.path,
            "file changed during formatting; no replacement was made for this file",
        ));
    }
    Ok(metadata.permissions())
}

fn write_files(files: &[FormattedFile]) -> Result<(), CliFormatError> {
    let mut staged = Vec::new();
    for file in files.iter().filter(|file| file.original != file.bytes) {
        let permissions = writable(file)?;
        let parent = file
            .path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let stage = || -> io::Result<TempPath> {
            let mut temporary = NamedTempFile::new_in(parent)?;
            temporary.write_all(&file.bytes)?;
            temporary.as_file().set_permissions(permissions)?;
            temporary.as_file().sync_all()?;
            // Keep cleanup ownership without holding one file descriptor per file.
            Ok(temporary.into_temp_path())
        };
        staged.push((file, stage().map_err(|err| error(&file.path, err))?));
    }
    // Catch edits during staging before the first replacement, then recheck
    // immediately before each atomic rename. This is not a batch transaction.
    for (file, _) in &staged {
        writable(file)?;
    }
    for (file, temporary) in staged {
        writable(file)?;
        temporary
            .persist(&file.path)
            .map_err(|err| error(&file.path, err.error))?;
    }
    Ok(())
}

use std::ffi::OsString;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Default)]
struct Stat {
    added: usize,
    deleted: usize,
    binary: Option<String>,
}

pub fn run() -> io::Result<()> {
    let mut root = capture(git(Path::new(".")).args(["rev-parse", "--show-toplevel"]))?;
    // Strip only Git's terminator, not newlines belonging to the directory name.
    if root.last() == Some(&b'\n') {
        root.pop();
    }
    let root = PathBuf::from(os_string(root)?);
    let status = capture(git(&root).args([
        "status",
        "--porcelain=v1",
        "-z",
        "--untracked-files=all",
        "--renames",
        "--ignore-submodules=none",
    ]))?;
    if status.is_empty() {
        return Ok(());
    }
    let head = git(&root)
        .args(["rev-parse", "--verify", "--quiet", "HEAD"])
        .output()?;
    let base = if head.status.success() {
        head.stdout
    } else if head.status.code() == Some(1) {
        capture(git(&root).args(["hash-object", "-t", "tree", "--stdin"]))?
    } else {
        return Err(io::Error::other("could not resolve HEAD"));
    };
    let base = String::from_utf8(base).map_err(io::Error::other)?;

    let mut changes = Vec::new();
    let mut records = status
        .split(|&b| b == 0)
        .filter(|record| !record.is_empty());
    while let Some(record) = records.next() {
        if record.len() < 4 || record[2] != b' ' {
            return Err(io::Error::other("invalid Git status record"));
        }
        let code = &record[..2];
        let path = &record[3..];
        let old = if code.iter().any(|b| *b == b'R' || *b == b'C') {
            Some(
                records
                    .next()
                    .ok_or_else(|| io::Error::other("missing rename source"))?,
            )
        } else {
            None
        };
        changes.push((code, path, old));
    }
    let mut rows = Vec::new();
    for &(code, path, old) in &changes {
        let name = match old {
            Some(old) => format!("{} -> {}", display_path(old), display_path(path)),
            None => display_path(path),
        };
        let untracked = code == b"??";
        let mut command = git(&root);
        command.args([
            "diff",
            "--numstat",
            "--stat",
            "-z",
            "--no-ext-diff",
            "--no-textconv",
            "--no-relative",
            "--no-color",
            "--no-exit-code",
        ]);
        if untracked {
            command.args(["--no-index", "--", "/dev/null"]);
        } else {
            command.args(["--find-renames", base.trim(), "--"]);
        }
        command.arg(os_string(path.to_vec())?);
        if let Some(old) = old {
            command.arg(os_string(old.to_vec())?);
        }
        // Git reports embedded untracked repositories as directories, not files.
        let directory = untracked && root.join(os_string(path.to_vec())?).is_dir();
        let stat = if directory {
            Stat::default()
        } else {
            let output = command.output()?;
            if !output.status.success()
                && !(untracked && output.status.code() == Some(1) && !output.stdout.is_empty())
            {
                return Err(io::Error::other(
                    String::from_utf8_lossy(&output.stderr).trim().to_owned(),
                ));
            }
            // A rename source can also have its own status row if it was reused.
            let removed_source = old.filter(|old| {
                !changes
                    .iter()
                    .any(|change| change.0 != b"??" && change.1 == *old)
            });
            parse_stat(&output.stdout, path, removed_source)?
        };
        let code = std::str::from_utf8(code).map_err(io::Error::other)?;
        rows.push((code.to_owned(), name, stat, directory));
    }

    let full_name_width = rows
        .iter()
        .map(|(_, name, _, _)| name.chars().count())
        .max()
        .unwrap_or(0);
    let max_lines = rows
        .iter()
        .map(|(_, _, stat, _)| stat.added + stat.deleted)
        .max()
        .unwrap_or(0);
    let count_width = max_lines.to_string().len();
    let columns = output_width();
    // Budget for the status, separators, counts, and a useful graph before
    // assigning space to paths. Short paths leave more room for the graph.
    let detail_width = rows
        .iter()
        .map(|(_, _, stat, directory)| {
            if *directory {
                "directory".len()
            } else {
                stat.binary.as_ref().map_or(0, String::len)
            }
        })
        .max()
        .unwrap_or(0);
    let preferred_graph = max_lines.min(50).min(columns / 4);
    let reserved = detail_width.max(count_width + 1 + preferred_graph);
    let name_width = full_name_width.min(columns.saturating_sub(6 + reserved).max(3));
    let graph_width = 50.min(columns.saturating_sub(7 + name_width + count_width));
    let terminal = if io::stdout().is_terminal() {
        "true"
    } else {
        "false"
    };
    let color = capture(git(&root).args(["config", "--get-colorbool", "color.diff", terminal]))?
        == b"true\n";
    let (green, red, reset) = if color {
        (
            git_color(&root, "color.diff.new", "green")?,
            git_color(&root, "color.diff.old", "red")?,
            "\x1b[m",
        )
    } else {
        (String::new(), String::new(), "")
    };
    let status_color =
        capture(git(&root).args(["config", "--get-colorbool", "color.status", terminal]))?
            == b"true\n";
    let status_colors = if status_color {
        [
            git_color(&root, "color.status.added", "green")?,
            git_color(&root, "color.status.changed", "red")?,
            git_color(&root, "color.status.untracked", "red")?,
            git_color(&root, "color.status.unmerged", "red")?,
        ]
    } else {
        Default::default()
    };
    let mut added = 0;
    let mut deleted = 0;
    let mut out = io::BufWriter::new(io::stdout().lock());
    for (code, name, stat, directory) in &rows {
        let name = shorten_path(name, name_width);
        let conflicted = code.contains('U') || matches!(code.as_str(), "AA" | "DD");
        for (column, letter) in code.chars().enumerate() {
            let color = &status_colors[if conflicted {
                3
            } else if letter == '?' {
                2
            } else {
                column
            }];
            if letter == ' ' || color.is_empty() {
                write!(out, "{letter}")?;
            } else {
                write!(out, "{color}{letter}\x1b[m")?;
            }
        }
        if code.contains('R') && stat.binary.is_none() && stat.added == 0 && stat.deleted == 0 {
            writeln!(out, " {name}")?;
            continue;
        }
        write!(out, " {name:name_width$} | ")?;
        if *directory {
            writeln!(out, "directory")?;
        } else if let Some(binary) = &stat.binary {
            if let Some((old, new)) = binary
                .strip_prefix("Bin ")
                .and_then(|sizes| sizes.strip_suffix(" bytes"))
                .and_then(|sizes| sizes.split_once(" -> "))
            {
                writeln!(out, "Bin {red}{old}{reset} -> {green}{new}{reset} bytes")?;
            } else {
                writeln!(out, "{binary}")?;
            }
        } else {
            let total = stat.added + stat.deleted;
            let width = if max_lines > graph_width {
                (total * graph_width).div_ceil(max_lines)
            } else {
                total
            };
            let (plus, minus) = if width == 0 {
                (0, 0)
            } else if stat.added == 0 {
                (0, width)
            } else if stat.deleted == 0 {
                (width, 0)
            } else {
                let width = width.max(2).min(graph_width);
                if width == 1 {
                    (
                        usize::from(stat.added >= stat.deleted),
                        usize::from(stat.added < stat.deleted),
                    )
                } else {
                    let plus = (stat.added * width / total).clamp(1, width - 1);
                    (plus, width - plus)
                }
            };
            write!(out, "{total:>count_width$}")?;
            if plus + minus > 0 {
                write!(out, " ")?;
                if plus > 0 {
                    write!(out, "{green}{}{reset}", "+".repeat(plus))?;
                }
                if minus > 0 {
                    write!(out, "{red}{}{reset}", "-".repeat(minus))?;
                }
            }
            writeln!(out)?;
        }
        added += stat.added;
        deleted += stat.deleted;
    }
    let files = rows.len();
    writeln!(
        out,
        " {files} {} changed, {green}{added} {}{reset}, {red}{deleted} {}{reset}",
        if files == 1 { "file" } else { "files" },
        if added == 1 {
            "insertion"
        } else {
            "insertions"
        },
        if deleted == 1 {
            "deletion"
        } else {
            "deletions"
        }
    )?;
    out.flush()
}

fn output_width() -> usize {
    if let Some(columns) = std::env::var("COLUMNS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|&columns| columns > 0)
    {
        return columns;
    }
    #[cfg(unix)]
    if io::stdout().is_terminal() {
        use std::os::fd::AsFd;
        // Query stdout's terminal, even when stdin is redirected. stty keeps
        // terminal-size detection portable across Unix without Rust dependencies.
        if let Ok(fd) = io::stdout().as_fd().try_clone_to_owned()
            && let Ok(output) = Command::new("stty")
                .arg("size")
                .stdin(Stdio::from(fd))
                .stderr(Stdio::null())
                .output()
            && output.status.success()
            && let Some(columns) = String::from_utf8_lossy(&output.stdout)
                .split_whitespace()
                .nth(1)
                .and_then(|value| value.parse::<usize>().ok())
                .filter(|&columns| columns > 0)
        {
            return columns;
        }
    }
    80
}

fn shorten_path(name: &str, width: usize) -> String {
    // display_path escapes all non-ASCII bytes, so bytes equal terminal cells.
    if name.len() <= width {
        return name.to_owned();
    }
    let suffix = &name[name.len() - width.saturating_sub(3)..];
    // Prefer a complete trailing path component, as Git's diffstat does.
    let suffix = suffix.find('/').map_or(suffix, |slash| &suffix[slash..]);
    format!("...{suffix}")
}

fn git_color(root: &Path, key: &str, default: &str) -> io::Result<String> {
    String::from_utf8(capture(git(root).args([
        "config",
        "--get-color",
        key,
        default,
    ]))?)
    .map_err(io::Error::other)
}

fn parse_stat(bytes: &[u8], destination: &[u8], removed_source: Option<&[u8]>) -> io::Result<Stat> {
    let mut stat = Stat::default();
    // Git emits NUL-delimited numstat records followed by the human-readable stat.
    // Only read size information from the latter; paths still come from numstat.
    let end = bytes.iter().rposition(|&b| b == 0).map_or(0, |i| i + 1);
    let summary = String::from_utf8_lossy(&bytes[end..]);
    let mut details = summary
        .lines()
        .filter_map(|line| line.rsplit_once(" | ").map(|(_, detail)| detail.trim()));
    let mut records = bytes[..end]
        .split(|&b| b == 0)
        .filter(|record| !record.is_empty());
    while let Some(record) = records.next() {
        let mut fields = record.splitn(3, |&b| b == b'\t');
        let added = fields.next().unwrap();
        let deleted = fields
            .next()
            .ok_or_else(|| io::Error::other("invalid Git numstat"))?;
        let mut path = fields
            .next()
            .ok_or_else(|| io::Error::other("missing numstat path"))?;
        let renamed = path.is_empty();
        if renamed {
            // -z uses two additional NUL-terminated paths for renames/no-index diffs.
            records
                .next()
                .ok_or_else(|| io::Error::other("missing numstat source"))?;
            path = records
                .next()
                .ok_or_else(|| io::Error::other("missing numstat destination"))?;
        }
        let detail = details.next();
        if path != destination && (renamed || Some(path) != removed_source) {
            continue;
        }
        if added == b"-" || deleted == b"-" {
            let detail = detail
                .filter(|detail| detail.starts_with("Bin "))
                .ok_or_else(|| io::Error::other("missing binary size information in Git stat"))?;
            stat.binary = Some(detail.to_owned());
        } else {
            stat.added += std::str::from_utf8(added)
                .map_err(io::Error::other)?
                .parse::<usize>()
                .map_err(io::Error::other)?;
            stat.deleted += std::str::from_utf8(deleted)
                .map_err(io::Error::other)?
                .parse::<usize>()
                .map_err(io::Error::other)?;
        }
    }
    Ok(stat)
}

// Keep each filename on one terminal-safe line, including arbitrary Unix bytes.
fn display_path(path: &[u8]) -> String {
    if path
        .iter()
        .all(|b| (32..127).contains(b) && *b != b'"' && *b != b'\\')
    {
        return String::from_utf8(path.to_vec()).unwrap();
    }
    let mut result = String::from("\"");
    for &b in path {
        match b {
            b'\n' => result.push_str("\\n"),
            b'\r' => result.push_str("\\r"),
            b'\t' => result.push_str("\\t"),
            b'"' => result.push_str("\\\""),
            b'\\' => result.push_str("\\\\"),
            32..=126 => result.push(char::from(b)),
            _ => result.push_str(&format!("\\{b:03o}")),
        }
    }
    result.push('"');
    result
}

fn git(root: &Path) -> Command {
    let mut command = Command::new("git");
    command
        .args(["--no-pager", "--literal-pathspecs"])
        .current_dir(root)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stdin(Stdio::null());
    command
}

fn capture(command: &mut Command) -> io::Result<Vec<u8>> {
    let output = command.output()?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(io::Error::other(
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        ))
    }
}

#[cfg(unix)]
fn os_string(bytes: Vec<u8>) -> io::Result<OsString> {
    use std::os::unix::ffi::OsStringExt;
    Ok(OsString::from_vec(bytes))
}

#[cfg(not(unix))]
fn os_string(bytes: Vec<u8>) -> io::Result<OsString> {
    String::from_utf8(bytes)
        .map(OsString::from)
        .map_err(io::Error::other)
}

use std::io::{self, IsTerminal, Write};
use std::process::{Command, Stdio};

pub fn run() -> io::Result<()> {
    let head = git()
        .args(["rev-parse", "--verify", "--quiet", "HEAD"])
        .output()?;
    if head.status.code() == Some(1) {
        // A repository with an unborn branch has no history yet.
        println!("spek — 0 commits");
        return Ok(());
    }
    if !head.status.success() {
        return Err(git_error(&head.stderr));
    }
    let revision = std::str::from_utf8(&head.stdout)
        .map_err(io::Error::other)?
        .trim();
    let history = capture(git().args([
        "log",
        "-z",
        "--no-color",
        "--no-patch",
        "--no-notes",
        "--no-show-signature",
        "--no-use-mailmap",
        "--decorate=full",
        "--encoding=none",
        "--date=format:%b %d, %Y|%H:%M:%S",
        "--format=%h%x00%s%x00%an%x00%ae%x00%ad%x00%D",
        revision,
        "--",
    ]))?;
    // Six NUL-terminated fields per commit, including an empty decoration field.
    // Lowercase %an/%ae and --encoding=none preserve the stored author identity.
    let data = history
        .strip_suffix(&[0])
        .ok_or_else(|| io::Error::other("invalid Git log output"))?;
    let fields: Vec<_> = data.split(|&b| b == 0).collect();
    if fields.len() % 6 != 0 {
        return Err(io::Error::other("invalid Git log record"));
    }
    let count = fields.len() / 6;
    let interactive = io::stdout().is_terminal();
    let terminal = if interactive { "true" } else { "false" };
    let color =
        capture(git().args(["config", "--get-colorbool", "color.ui", terminal]))? == b"true\n";
    let (hash_color, subject_color, date_color, bullet_color, reset) = if color {
        ("\x1b[37m", "\x1b[97m", "\x1b[33m", "\x1b[36m", "\x1b[m")
    } else {
        ("", "", "", "", "")
    };
    let metadata_color = if color { "\x1b[37m" } else { "" };
    let subject_bold = "";
    let mut out = Vec::new();
    writeln!(
        out,
        "spek — {count} {}",
        if count == 1 { "commit" } else { "commits" }
    )?;
    let mut previous_date = String::new();
    for commit in fields.chunks_exact(6) {
        let [hash, subject, author, email, date, refs] = commit else {
            unreachable!()
        };
        let (date, time) = std::str::from_utf8(date)
            .map_err(io::Error::other)?
            .split_once('|')
            .ok_or_else(|| io::Error::other("invalid Git log date"))?;
        let date = date.replace(" 0", " ");
        writeln!(out)?;
        if date != previous_date {
            writeln!(out, "{date_color}{date}{reset}")?;
            previous_date = date;
        }
        write!(out, " {bullet_color}•{reset} {subject_bold}{subject_color}")?;
        write_subject(&mut out, subject, color, subject_color)?;
        write!(out, "{reset}{hash_color}")?;
        out.write_all(b"  ")?;
        out.write_all(hash)?;
        write!(out, "{reset}\n   {metadata_color}{time} · ")?;
        out.write_all(author)?;
        write!(out, " <")?;
        out.write_all(email)?;
        write!(out, ">{reset}")?;
        if !refs.is_empty() {
            write!(out, " · ")?;
            write_decorations(&mut out, refs, color)?;
        }
        writeln!(out, "{reset}")?;
    }
    display(&out, interactive)
}

fn write_subject(
    out: &mut impl Write,
    subject: &[u8],
    color: bool,
    subject_color: &str,
) -> io::Result<()> {
    if !color {
        return out.write_all(subject);
    }
    let mut start = 0;
    let mut index = 0;
    while index < subject.len() {
        if subject[index] == b'#' && subject.get(index + 1).is_some_and(u8::is_ascii_digit) {
            out.write_all(&subject[start..index])?;
            let mut end = index + 2;
            while subject.get(end).is_some_and(u8::is_ascii_digit) {
                end += 1;
            }
            write!(out, "\x1b[36m")?;
            out.write_all(&subject[index..end])?;
            // Restore the subject's normal foreground while keeping it bold.
            write!(out, "\x1b[m{subject_color}")?;
            start = end;
            index = end;
        } else {
            index += 1;
        }
    }
    out.write_all(&subject[start..])
}

fn write_decorations(out: &mut impl Write, refs: &[u8], color: bool) -> io::Result<()> {
    // Full ref names distinguish remote refs from local branches containing '/'.
    // Ref names cannot contain spaces, so Git's ", " and " -> " are unambiguous.
    let mut remaining = refs;
    loop {
        let end = remaining.windows(2).position(|bytes| bytes == b", ");
        let item = end.map_or(remaining, |end| &remaining[..end]);
        if let Some(branch) = item.strip_prefix(b"HEAD -> ") {
            write_ref(out, b"HEAD", "\x1b[36m", color)?;
            write!(out, " -> ")?;
            write_decoration(out, branch, color)?;
        } else {
            write_decoration(out, item, color)?;
        }
        match end {
            Some(end) => {
                write!(out, ", ")?;
                remaining = &remaining[end + 2..];
            }
            None => return Ok(()),
        }
    }
}

fn write_decoration(out: &mut impl Write, name: &[u8], color: bool) -> io::Result<()> {
    if name == b"HEAD" {
        write_ref(out, name, "\x1b[36m", color)
    } else if let Some(name) = name.strip_prefix(b"refs/heads/") {
        write_ref(out, name, "\x1b[32m", color)
    } else if let Some(name) = name.strip_prefix(b"refs/remotes/") {
        write_ref(out, name, "\x1b[31m", color)
    } else if let Some(name) = name.strip_prefix(b"tag: refs/tags/") {
        write!(out, "tag: ")?;
        write_ref(out, name, "\x1b[33m", color)
    } else {
        out.write_all(name)
    }
}

fn write_ref(out: &mut impl Write, name: &[u8], style: &str, color: bool) -> io::Result<()> {
    if color {
        out.write_all(style.as_bytes())?;
    }
    out.write_all(name)?;
    if color {
        write!(out, "\x1b[m")?;
    }
    Ok(())
}

fn display(output: &[u8], interactive: bool) -> io::Result<()> {
    if interactive {
        // Git resolves GIT_PAGER, core.pager, PAGER, then its default (usually less).
        // Do not use git(): --no-pager would force this query to return "cat".
        let pager = capture(
            Command::new("git")
                .args(["var", "GIT_PAGER"])
                .stdin(Stdio::null()),
        )?;
        let pager = std::str::from_utf8(&pager)
            .map_err(io::Error::other)?
            .trim_end_matches('\n');
        if !pager.is_empty() && pager != "cat" {
            // Pager settings are shell commands, just as they are in Git.
            let mut command = Command::new("sh");
            command.args(["-c", pager]).stdin(Stdio::piped());
            if std::env::var_os("LESS").is_none() {
                command.env("LESS", "FRX");
            }
            let mut child = command.spawn()?;
            let result = child.stdin.take().unwrap().write_all(output);
            let status = child.wait()?;
            if !status.success() {
                return Err(io::Error::other(format!("pager exited with {status}")));
            }
            return match result {
                // Quitting the pager early closes its input; this is normal.
                Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(()),
                result => result,
            };
        }
    }
    let mut out = io::stdout().lock();
    out.write_all(output)?;
    out.flush()
}

fn git() -> Command {
    let mut command = Command::new("git");
    command
        .arg("--no-pager")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("LC_ALL", "C")
        .stdin(Stdio::null());
    command
}

fn capture(command: &mut Command) -> io::Result<Vec<u8>> {
    let output = command.output()?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(git_error(&output.stderr))
    }
}

fn git_error(stderr: &[u8]) -> io::Error {
    io::Error::other(String::from_utf8_lossy(stderr).trim().to_owned())
}

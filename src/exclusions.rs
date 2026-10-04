use crate::diff;
use std::env;
use std::ffi::{OsStr, OsString};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

struct Repository {
    root: Vec<u8>,
    excluded: Vec<Vec<u8>>,
}

#[derive(Default)]
struct State {
    repositories: Vec<Repository>,
}

pub fn paths(root: &Path) -> io::Result<Vec<Vec<u8>>> {
    let root = path_bytes(&fs::canonicalize(root)?)?;
    Ok(read_state()?
        .repositories
        .into_iter()
        .find(|repository| repository.root == root)
        .map_or_else(Vec::new, |repository| repository.excluded))
}

pub fn exclude(arguments: Vec<OsString>) -> io::Result<()> {
    change(arguments, true)
}

pub fn include(arguments: Vec<OsString>) -> io::Result<()> {
    change(arguments, false)
}

pub fn include_all() -> io::Result<()> {
    let root = canonical_root()?;
    let mut state = read_state()?;
    let before = state.repositories.len();
    state
        .repositories
        .retain(|repository| repository.root != root);
    if state.repositories.len() != before {
        save_state(&state)?;
    }
    println!("Included all paths.");
    Ok(())
}

pub fn list() -> io::Result<()> {
    let root = canonical_root()?;
    let state = read_state()?;
    if let Some(repository) = state
        .repositories
        .iter()
        .find(|repository| repository.root == root)
    {
        for path in &repository.excluded {
            println!("{}", display_path(path));
        }
    }
    Ok(())
}

fn change(arguments: Vec<OsString>, excluding: bool) -> io::Result<()> {
    let root = canonical_root()?;
    let paths: Vec<_> = arguments
        .iter()
        .map(|path| repository_relative_path(path, &root))
        .collect::<io::Result<_>>()?;
    let count = paths.len();
    let mut state = read_state()?;
    let repository_index = state
        .repositories
        .iter()
        .position(|repository| repository.root == root);
    let mut changed = false;
    if excluding {
        let repository = match repository_index {
            Some(index) => &mut state.repositories[index],
            None => {
                state.repositories.push(Repository {
                    root,
                    excluded: Vec::new(),
                });
                state.repositories.last_mut().unwrap()
            }
        };
        for path in paths {
            if !repository.excluded.contains(&path) {
                repository.excluded.push(path);
                changed = true;
            }
        }
        repository.excluded.sort();
    } else if let Some(index) = repository_index {
        let repository = &mut state.repositories[index];
        let before = repository.excluded.len();
        repository.excluded.retain(|path| !paths.contains(path));
        changed = repository.excluded.len() != before;
        if repository.excluded.is_empty() {
            state.repositories.remove(index);
        }
    }
    if changed {
        save_state(&state)?;
    }
    let verb = if excluding { "Excluded" } else { "Included" };
    println!(
        "{verb} {count} {}.",
        if count == 1 { "path" } else { "paths" }
    );
    Ok(())
}

fn canonical_root() -> io::Result<Vec<u8>> {
    path_bytes(&fs::canonicalize(diff::repository_root()?)?)
}

fn repository_relative_path(argument: &OsStr, root: &[u8]) -> io::Result<Vec<u8>> {
    let root = path_from_bytes(root)?;
    let current = fs::canonicalize(env::current_dir()?)?;
    let path = Path::new(argument);
    let absolute = if path.is_absolute() {
        path.to_owned()
    } else {
        current.join(path)
    };
    let normalized = normalize(&absolute);
    let relative = normalized
        .strip_prefix(&root)
        .map_err(|_| io::Error::other("excluded paths must be inside the current repository"))?;
    if relative.as_os_str().is_empty() {
        return Err(io::Error::other(
            "excluded path must name a file or directory",
        ));
    }
    path_bytes(relative)
}

fn normalize(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::RootDir | Component::Prefix(_) | Component::Normal(_) => {
                normalized.push(component.as_os_str());
            }
        }
    }
    normalized
}

fn state_path() -> io::Result<PathBuf> {
    let directory = match env::var_os("XDG_STATE_HOME") {
        Some(path) if !path.is_empty() => PathBuf::from(path),
        _ => PathBuf::from(env::var_os("HOME").ok_or_else(|| {
            io::Error::other("HOME is not set; set XDG_STATE_HOME for Spek exclusions")
        })?)
        .join(".local/state"),
    };
    Ok(directory.join("spek/exclusions-v1.json"))
}

fn read_state() -> io::Result<State> {
    let path = state_path()?;
    match fs::read(&path) {
        Ok(bytes) => parse_state(&bytes).map_err(|message| {
            io::Error::other(format!(
                "invalid exclusion state {}: {message}",
                path.display()
            ))
        }),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(State::default()),
        Err(error) => Err(error),
    }
}

fn save_state(state: &State) -> io::Result<()> {
    let path = state_path()?;
    if state.repositories.is_empty() {
        return match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        };
    }
    let parent = path.parent().unwrap();
    fs::create_dir_all(parent)?;
    let temporary = parent.join(format!(
        ".exclusions-v1-{}-{}.tmp",
        std::process::id(),
        NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
    ));
    let mut file = create_private_file(&temporary)?;
    file.write_all(render_state(state).as_bytes())?;
    file.sync_all()?;
    drop(file);
    fs::rename(temporary, path)
}

#[cfg(unix)]
fn create_private_file(path: &Path) -> io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
}

#[cfg(not(unix))]
fn create_private_file(path: &Path) -> io::Result<File> {
    OpenOptions::new().write(true).create_new(true).open(path)
}

fn render_state(state: &State) -> String {
    let mut output = String::from("{\n  \"version\": 1,\n  \"repositories\": [");
    for (repository_index, repository) in state.repositories.iter().enumerate() {
        if repository_index > 0 {
            output.push(',');
        }
        output.push_str("\n    {\n      \"root\": ");
        output.push_str(&json_bytes(&repository.root));
        output.push_str(",\n      \"excluded\": [");
        for (path_index, path) in repository.excluded.iter().enumerate() {
            if path_index > 0 {
                output.push(',');
            }
            output.push_str("\n        ");
            output.push_str(&json_bytes(path));
        }
        if !repository.excluded.is_empty() {
            output.push('\n');
        }
        output.push_str("      ]\n    }");
    }
    if !state.repositories.is_empty() {
        output.push('\n');
    }
    output.push_str("  ]\n}\n");
    output
}

fn json_bytes(bytes: &[u8]) -> String {
    let mut output = String::from("\"");
    for &byte in bytes {
        match byte {
            b'\"' => output.push_str("\\\""),
            b'\\' => output.push_str("\\\\"),
            0x20..=0x7e => output.push(char::from(byte)),
            _ => output.push_str(&format!("\\u00{byte:02x}")),
        }
    }
    output.push('\"');
    output
}

fn parse_state(bytes: &[u8]) -> Result<State, &'static str> {
    let mut parser = Parser { bytes, position: 0 };
    parser.whitespace();
    parser.literal(b"{")?;
    parser.key(b"version")?;
    if parser.number()? != 1 {
        return Err("unsupported version");
    }
    parser.literal(b",")?;
    parser.key(b"repositories")?;
    parser.literal(b"[")?;
    let mut repositories = Vec::new();
    if !parser.next_is(b']') {
        loop {
            parser.literal(b"{")?;
            parser.key(b"root")?;
            let root = parser.string()?;
            parser.literal(b",")?;
            parser.key(b"excluded")?;
            parser.literal(b"[")?;
            let mut excluded = Vec::new();
            if !parser.next_is(b']') {
                loop {
                    excluded.push(parser.string()?);
                    if parser.next_is(b']') {
                        break;
                    }
                    parser.literal(b",")?;
                }
            }
            parser.literal(b"]")?;
            parser.literal(b"}")?;
            repositories.push(Repository { root, excluded });
            if parser.next_is(b']') {
                break;
            }
            parser.literal(b",")?;
        }
    }
    parser.literal(b"]")?;
    parser.literal(b"}")?;
    parser.whitespace();
    if parser.position != bytes.len() {
        return Err("unexpected content");
    }
    Ok(State { repositories })
}

struct Parser<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl Parser<'_> {
    fn whitespace(&mut self) {
        while self
            .bytes
            .get(self.position)
            .is_some_and(u8::is_ascii_whitespace)
        {
            self.position += 1;
        }
    }

    fn literal(&mut self, expected: &[u8]) -> Result<(), &'static str> {
        self.whitespace();
        if self
            .bytes
            .get(self.position..self.position + expected.len())
            != Some(expected)
        {
            return Err("unexpected JSON token");
        }
        self.position += expected.len();
        Ok(())
    }

    fn key(&mut self, expected: &[u8]) -> Result<(), &'static str> {
        if self.string()? != expected {
            return Err("unexpected JSON key");
        }
        self.literal(b":")
    }

    fn number(&mut self) -> Result<u8, &'static str> {
        self.whitespace();
        let number = *self.bytes.get(self.position).ok_or("missing number")?;
        self.position += 1;
        if number.is_ascii_digit() {
            Ok(number - b'0')
        } else {
            Err("invalid number")
        }
    }

    fn next_is(&mut self, expected: u8) -> bool {
        self.whitespace();
        self.bytes.get(self.position) == Some(&expected)
    }

    fn string(&mut self) -> Result<Vec<u8>, &'static str> {
        self.whitespace();
        if self.bytes.get(self.position) != Some(&b'\"') {
            return Err("expected JSON string");
        }
        self.position += 1;
        let mut result = Vec::new();
        loop {
            let byte = *self
                .bytes
                .get(self.position)
                .ok_or("unterminated JSON string")?;
            self.position += 1;
            match byte {
                b'\"' => return Ok(result),
                b'\\' => {
                    let escaped = *self.bytes.get(self.position).ok_or("invalid JSON escape")?;
                    self.position += 1;
                    match escaped {
                        b'\"' | b'\\' => result.push(escaped),
                        b'u' => {
                            if self.bytes.get(self.position..self.position + 2) != Some(b"00") {
                                return Err("only byte JSON escapes are supported");
                            }
                            let high = hex(*self
                                .bytes
                                .get(self.position + 2)
                                .ok_or("short JSON escape")?)?;
                            let low = hex(*self
                                .bytes
                                .get(self.position + 3)
                                .ok_or("short JSON escape")?)?;
                            self.position += 4;
                            result.push(high * 16 + low);
                        }
                        _ => return Err("invalid JSON escape"),
                    }
                }
                0x00..=0x1f => return Err("unescaped control character"),
                _ => result.push(byte),
            }
        }
    }
}

fn hex(byte: u8) -> Result<u8, &'static str> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err("invalid JSON escape"),
    }
}

fn display_path(path: &[u8]) -> String {
    if path
        .iter()
        .all(|byte| (32..127).contains(byte) && *byte != b'\"' && *byte != b'\\')
    {
        return String::from_utf8(path.to_vec()).unwrap();
    }
    let mut result = String::from("\"");
    for &byte in path {
        match byte {
            b'\n' => result.push_str("\\n"),
            b'\r' => result.push_str("\\r"),
            b'\t' => result.push_str("\\t"),
            b'\"' => result.push_str("\\\""),
            b'\\' => result.push_str("\\\\"),
            32..=126 => result.push(char::from(byte)),
            _ => result.push_str(&format!("\\{byte:03o}")),
        }
    }
    result.push('\"');
    result
}

#[cfg(unix)]
fn path_bytes(path: &Path) -> io::Result<Vec<u8>> {
    use std::os::unix::ffi::OsStrExt;
    Ok(path.as_os_str().as_bytes().to_vec())
}

#[cfg(not(unix))]
fn path_bytes(path: &Path) -> io::Result<Vec<u8>> {
    path.to_str()
        .map(|path| path.as_bytes().to_vec())
        .ok_or_else(|| io::Error::other("path is not UTF-8"))
}

#[cfg(unix)]
fn path_from_bytes(bytes: &[u8]) -> io::Result<PathBuf> {
    use std::os::unix::ffi::OsStringExt;
    Ok(PathBuf::from(OsString::from_vec(bytes.to_vec())))
}

#[cfg(not(unix))]
fn path_from_bytes(bytes: &[u8]) -> io::Result<PathBuf> {
    String::from_utf8(bytes.to_vec())
        .map(PathBuf::from)
        .map_err(io::Error::other)
}

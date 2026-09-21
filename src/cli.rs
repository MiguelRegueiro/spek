use std::{env, io};

pub enum Command {
    Diff,
    Log,
}

pub fn parse() -> io::Result<Option<Command>> {
    let args: Vec<_> = env::args_os().skip(1).collect();
    match args.as_slice() {
        [] => Ok(Some(Command::Diff)),
        [arg] if arg == "diff" => Ok(Some(Command::Diff)),
        [arg] if arg == "log" => Ok(Some(Command::Log)),
        [arg] if arg == "--help" || arg == "-h" => {
            println!(
                "Usage: spek [diff|log]\n       spek [OPTIONS]\n\nCommands:\n  diff  Show compact status and diff stats, including untracked files (default)\n  log   Show compact commit history\n\nOptions:\n  -h, --help     Print help\n  -V, --version  Print version"
            );
            Ok(None)
        }
        [arg] if arg == "--version" || arg == "-V" => {
            println!("spek {}", env!("CARGO_PKG_VERSION"));
            Ok(None)
        }
        _ => Err(io::Error::other(
            "usage: spek [diff|log] | --help | --version",
        )),
    }
}

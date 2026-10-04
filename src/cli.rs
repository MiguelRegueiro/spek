use std::{env, ffi::OsString, io};

pub enum Command {
    Diff,
    Log,
    Exclude(Vec<OsString>),
    ExcludeList,
    Include(Vec<OsString>),
    IncludeAll,
}

pub fn parse() -> io::Result<Option<Command>> {
    let args: Vec<_> = env::args_os().skip(1).collect();
    match args.as_slice() {
        [] => Ok(Some(Command::Diff)),
        [arg] if arg == "diff" => Ok(Some(Command::Diff)),
        [arg] if arg == "log" => Ok(Some(Command::Log)),
        [command, option] if command == "exclude" && option == "--list" => {
            Ok(Some(Command::ExcludeList))
        }
        [command, paths @ ..] if command == "exclude" && !paths.is_empty() => {
            Ok(Some(Command::Exclude(paths.to_vec())))
        }
        [command, option] if command == "include" && option == "all" => {
            Ok(Some(Command::IncludeAll))
        }
        [command, paths @ ..] if command == "include" && !paths.is_empty() => {
            Ok(Some(Command::Include(paths.to_vec())))
        }
        [arg] if arg == "--help" || arg == "-h" => {
            println!(
                "Usage: spek [diff|log]\n       spek exclude <path>...\n       spek exclude --list\n       spek include <path>...\n       spek include all\n       spek [OPTIONS]\n\nCommands:\n  diff               Show compact status and diff stats, including untracked files (default)\n  log                Show compact commit history\n  exclude <path>...  Hide paths from Spek's diff view for this checkout\n  exclude --list     List paths hidden from Spek's diff view\n  include <path>...  Show previously excluded paths\n  include all        Clear all hidden paths for this checkout\n\nExcluded paths are stored in Spek's user-local state. They do not modify\nthe repository, Git configuration, index, or ignore files.\n\nOptions:\n  -h, --help         Print help\n  -V, --version      Print version"
            );
            Ok(None)
        }
        [arg] if arg == "--version" || arg == "-V" => {
            println!("spek {}", env!("CARGO_PKG_VERSION"));
            Ok(None)
        }
        _ => Err(io::Error::other(
            "usage: spek [diff|log] | exclude <path>... | include <path>... | --help | --version",
        )),
    }
}

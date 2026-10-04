mod cli;
mod diff;
mod exclusions;
mod log;

use std::io;
use std::process::ExitCode;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("spek: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> io::Result<()> {
    match cli::parse()? {
        Some(cli::Command::Diff) => diff::run(),
        Some(cli::Command::Log) => log::run(),
        Some(cli::Command::Exclude(paths)) => exclusions::exclude(paths),
        Some(cli::Command::ExcludeList) => exclusions::list(),
        Some(cli::Command::Include(paths)) => exclusions::include(paths),
        Some(cli::Command::IncludeAll) => exclusions::include_all(),
        None => Ok(()),
    }
}

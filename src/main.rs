mod cli;
mod diff;
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
        None => Ok(()),
    }
}

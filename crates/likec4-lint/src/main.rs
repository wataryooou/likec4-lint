//! `likec4-lint`: a fast linter and formatter for the LikeC4 DSL.

mod cli;
mod commands;
mod config;
mod discover;
mod report;

use std::process::ExitCode;

use clap::Parser;

fn main() -> ExitCode {
    reset_sigpipe();
    let cli = cli::Cli::parse();

    let result = match &cli.command {
        cli::Command::Lint(args) => commands::lint::run(args),
        cli::Command::Format(args) => commands::format::run(args),
        cli::Command::Check(args) => commands::check::run(args),
    };

    match result {
        Ok(code) => ExitCode::from(code),
        Err(err) => {
            eprintln!("error: {err:#}");
            ExitCode::from(2)
        }
    }
}

/// Let `likec4-lint ... | head` terminate quietly instead of panicking on a closed pipe.
#[cfg(unix)]
fn reset_sigpipe() {
    // SAFETY: resetting the SIGPIPE disposition to the default is what every other
    // process expects from a well-behaved CLI; it does not touch any Rust-owned state.
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
}

#[cfg(not(unix))]
fn reset_sigpipe() {}

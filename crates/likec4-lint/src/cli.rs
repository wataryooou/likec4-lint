//! Command-line argument definitions (`clap` derive).

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

use crate::report::{ColorMode, OutputFormat};

#[derive(Parser, Debug)]
#[command(
    name = "likec4-lint",
    version,
    about = "Fast linter and formatter for the LikeC4 DSL",
    propagate_version = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Lint LikeC4 documents
    Lint(LintArgs),
    /// Format LikeC4 documents
    Format(FormatArgs),
    /// Lint and check formatting (`lint` + `format --check`)
    Check(CheckArgs),
}

/// Options shared by every subcommand.
#[derive(Args, Debug, Clone)]
pub struct CommonArgs {
    /// Files or directories to process (defaults to the current directory)
    #[arg(value_name = "PATHS")]
    pub paths: Vec<PathBuf>,

    /// Path to the config file (default: search upwards from the current directory for `likec4-lint.toml`)
    #[arg(long, value_name = "FILE")]
    pub config: Option<PathBuf>,

    /// Diagnostics output format
    #[arg(long, value_enum, default_value = "pretty")]
    pub format: OutputFormat,

    /// Shorthand for `--format json`
    #[arg(long, conflicts_with = "format")]
    pub json: bool,

    /// Color output
    #[arg(long, value_enum, default_value = "auto")]
    pub color: ColorMode,

    /// Only print diagnostics: suppress the per-file status lines and the summary line
    #[arg(long)]
    pub quiet: bool,
}

impl CommonArgs {
    /// Effective output format, taking the `--json` shorthand into account.
    pub fn output_format(&self) -> OutputFormat {
        if self.json {
            OutputFormat::Json
        } else {
            self.format
        }
    }
}

#[derive(Args, Debug, Clone)]
pub struct LintArgs {
    #[command(flatten)]
    pub common: CommonArgs,

    /// List all available lint rules and exit
    #[arg(long)]
    pub list_rules: bool,
}

#[derive(Args, Debug, Clone)]
pub struct FormatArgs {
    #[command(flatten)]
    pub common: CommonArgs,

    /// Check whether files are formatted, without writing changes
    #[arg(long)]
    pub check: bool,

    /// Write formatted output back to files
    #[arg(long, conflicts_with = "check")]
    pub write: bool,

    /// Read a single document from stdin and print the formatted result to stdout
    /// (with --check: exit 1 when the input is not formatted, print nothing)
    #[arg(long, conflicts_with_all = ["write", "paths"])]
    pub stdin: bool,

    /// File name shown in messages for --stdin input (default: `<stdin>`)
    #[arg(long, value_name = "PATH", requires = "stdin")]
    pub stdin_filepath: Option<PathBuf>,

    /// Quote style [possible values: auto, single, double, ignore]
    #[arg(long, value_name = "STYLE")]
    pub quote_style: Option<String>,

    /// Indentation width, in spaces
    #[arg(long, value_name = "N")]
    pub indent_width: Option<usize>,

    /// Indent with tabs instead of spaces
    #[arg(long)]
    pub use_tabs: bool,

    /// Show a unified diff for files that need formatting (with --check)
    #[arg(long)]
    pub diff: bool,
}

#[derive(Args, Debug, Clone)]
pub struct CheckArgs {
    #[command(flatten)]
    pub common: CommonArgs,
}

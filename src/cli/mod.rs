pub mod global_args;
pub mod read_repo;
pub mod sync;
pub mod this_repo;

use crate::cli::global_args::GlobalArgs;
use crate::cli::read_repo::ReadRepoArgs;
use crate::cli::sync::SyncArgs;
use crate::cli::this_repo::ThisRepoArgs;
use arbitrary::Arbitrary;
use eyre::Context;
use facet::Facet;
use figue::FigueBuiltins;
use figue::{self as args};
use std::ffi::OsString;

/// Trait for converting CLI structures to command line arguments.
pub trait ToArgs {
    /// Convert the CLI structure to command line arguments.
    fn to_args(&self) -> Vec<OsString> {
        Vec::new()
    }
}

// Blanket implementation for references
impl<T: ToArgs> ToArgs for &T {
    fn to_args(&self) -> Vec<OsString> {
        (*self).to_args()
    }
}

/// A demonstration command line utility.
#[derive(Facet, Arbitrary, Debug)]
pub struct Cli {
    /// Global arguments (`debug`, `log_filter`, `log_file`).
    #[facet(flatten)]
    pub global: GlobalArgs,

    /// Standard CLI options (help, version, completions).
    #[facet(flatten)]
    #[arbitrary(default)]
    pub builtins: FigueBuiltins,

    /// The command to run.
    #[facet(args::subcommand)]
    pub command: Command,
}

impl PartialEq for Cli {
    fn eq(&self, other: &Self) -> bool {
        // Ignore builtins in comparison since FigueBuiltins doesn't implement PartialEq
        self.global == other.global && self.command == other.command
    }
}

impl Cli {
    /// # Errors
    ///
    /// This function will return an error if the tokio runtime cannot be built or if the command fails.
    pub fn invoke(self) -> eyre::Result<()> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .wrap_err("Failed to build tokio runtime")?;
        runtime.block_on(async move { self.command.invoke().await })?;
        Ok(())
    }
}

impl ToArgs for Cli {
    fn to_args(&self) -> Vec<OsString> {
        let mut args = Vec::new();
        args.extend(self.global.to_args());
        args.extend(self.command.to_args());
        args
    }
}

/// A demonstration command line utility.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[repr(u8)]
pub enum Command {
    /// Read-repo configuration commands.
    ReadRepo(ReadRepoArgs),
    /// This-repo configuration commands.
    ThisRepo(ThisRepoArgs),
    /// Synchronize vanity commits from configured read repos into the configured this-repo.
    ///
    /// Reads commit history from every repo in `read-repo list`, then ensures each source commit
    /// has a corresponding empty vanity commit in `this-repo`.
    Sync(SyncArgs),
}

impl Command {
    /// # Errors
    ///
    /// This function will return an error if the subcommand fails.
    pub async fn invoke(self) -> eyre::Result<()> {
        match self {
            Command::ReadRepo(args) => args.invoke().await,
            Command::ThisRepo(args) => args.invoke().await,
            Command::Sync(args) => args.invoke().await,
        }
    }
}

impl ToArgs for Command {
    fn to_args(&self) -> Vec<OsString> {
        let mut args = Vec::new();
        match self {
            Command::ReadRepo(read_repo_args) => {
                args.push("read-repo".into());
                args.extend(read_repo_args.to_args());
            }
            Command::ThisRepo(this_repo_args) => {
                args.push("this-repo".into());
                args.extend(this_repo_args.to_args());
            }
            Command::Sync(sync_args) => {
                args.push("sync".into());
                args.extend(sync_args.to_args());
            }
        }
        args
    }
}

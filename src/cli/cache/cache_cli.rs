use crate::cli::ToArgs;
use crate::cli::cache::clean::CacheCleanArgs;
use crate::cli::cache::path::CachePathArgs;
use arbitrary::Arbitrary;
use eyre::Result;
use facet::Facet;
use figue as args;
use std::ffi::OsString;

/// Cache-related commands.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
pub struct CacheArgs {
    /// The cache subcommand to run.
    #[facet(args::subcommand)]
    pub command: CacheCommand,
}

/// Cache subcommands.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[repr(u8)]
pub enum CacheCommand {
    /// Clean the cache.
    Clean(CacheCleanArgs),
    /// Show or manage cache paths.
    Path(CachePathArgs),
}

impl CacheArgs {
    /// # Errors
    ///
    /// This function will return an error if the subcommand fails.
    pub async fn invoke(self) -> Result<()> {
        match self.command {
            CacheCommand::Clean(args) => args.invoke().await?,
            CacheCommand::Path(args) => args.invoke().await?,
        }

        Ok(())
    }
}

impl ToArgs for CacheArgs {
    fn to_args(&self) -> Vec<OsString> {
        let mut args = Vec::new();
        match &self.command {
            CacheCommand::Clean(clean_args) => {
                args.push("clean".into());
                args.extend(clean_args.to_args());
            }
            CacheCommand::Path(path_args) => {
                args.push("path".into());
                args.extend(path_args.to_args());
            }
        }
        args
    }
}

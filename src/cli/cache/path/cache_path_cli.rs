use crate::cli::ToArgs;
use crate::cli::cache::path::show::CachePathShowArgs;
use arbitrary::Arbitrary;
use eyre::Result;
use facet::Facet;
use figue as args;
use std::ffi::OsString;

/// Cache path commands.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
pub struct CachePathArgs {
    /// The cache path subcommand to run.
    #[facet(args::subcommand)]
    pub command: CachePathCommand,
}

/// Cache path subcommands.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[repr(u8)]
pub enum CachePathCommand {
    /// Show the cache path.
    Show(CachePathShowArgs),
}

impl CachePathArgs {
    /// # Errors
    ///
    /// This function will return an error if the subcommand fails.
    pub async fn invoke(self) -> Result<()> {
        match self.command {
            CachePathCommand::Show(args) => args.invoke().await?,
        }

        Ok(())
    }
}

impl ToArgs for CachePathArgs {
    fn to_args(&self) -> Vec<OsString> {
        let mut args = Vec::new();
        match &self.command {
            CachePathCommand::Show(show_args) => {
                args.push("show".into());
                args.extend(show_args.to_args());
            }
        }
        args
    }
}

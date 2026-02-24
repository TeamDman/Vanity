use crate::cli::ToArgs;
use crate::cli::home::path::HomePathArgs;
use arbitrary::Arbitrary;
use eyre::Result;
use facet::Facet;
use figue as args;
use std::ffi::OsString;

/// Home-related commands.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
pub struct HomeArgs {
    /// The home subcommand to run.
    #[facet(args::subcommand)]
    pub command: HomeCommand,
}

/// Home subcommands.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[repr(u8)]
pub enum HomeCommand {
    /// Show or manage home paths.
    Path(HomePathArgs),
}

impl HomeArgs {
    /// # Errors
    ///
    /// This function will return an error if the subcommand fails.
    pub async fn invoke(self) -> Result<()> {
        match self.command {
            HomeCommand::Path(args) => args.invoke().await?,
        }

        Ok(())
    }
}

impl ToArgs for HomeArgs {
    fn to_args(&self) -> Vec<OsString> {
        let mut args = Vec::new();
        match &self.command {
            HomeCommand::Path(path_args) => {
                args.push("path".into());
                args.extend(path_args.to_args());
            }
        }
        args
    }
}

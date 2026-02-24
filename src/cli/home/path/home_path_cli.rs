use crate::cli::ToArgs;
use crate::cli::home::path::show::HomePathShowArgs;
use arbitrary::Arbitrary;
use eyre::Result;
use facet::Facet;
use figue as args;
use std::ffi::OsString;

/// Home path commands.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
pub struct HomePathArgs {
    /// The home path subcommand to run.
    #[facet(args::subcommand)]
    pub command: HomePathCommand,
}

/// Home path subcommands.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[repr(u8)]
pub enum HomePathCommand {
    /// Show the home path.
    Show(HomePathShowArgs),
}

impl HomePathArgs {
    /// # Errors
    ///
    /// This function will return an error if the subcommand fails.
    pub async fn invoke(self) -> Result<()> {
        match self.command {
            HomePathCommand::Show(args) => args.invoke().await?,
        }

        Ok(())
    }
}

impl ToArgs for HomePathArgs {
    fn to_args(&self) -> Vec<OsString> {
        let mut args = Vec::new();
        match &self.command {
            HomePathCommand::Show(show_args) => {
                args.push("show".into());
                args.extend(show_args.to_args());
            }
        }
        args
    }
}

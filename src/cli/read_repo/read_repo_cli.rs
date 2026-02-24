use crate::cli::ToArgs;
use crate::vanity::VanityConfig;
use arbitrary::Arbitrary;
use eyre::Result;
use facet::Facet;
use figue as args;
use std::ffi::OsString;
use std::path::PathBuf;

#[derive(Facet, Arbitrary, Debug, PartialEq)]
pub struct ReadRepoArgs {
    #[facet(args::subcommand)]
    pub command: ReadRepoCommand,
}

#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[repr(u8)]
pub enum ReadRepoCommand {
    Add(ReadRepoAddArgs),
    List(ReadRepoListArgs),
}

#[derive(Facet, Arbitrary, Debug, PartialEq)]
pub struct ReadRepoAddArgs {
    #[facet(args::positional)]
    pub path: PathBuf,
}

#[derive(Facet, Arbitrary, Debug, PartialEq, Default)]
pub struct ReadRepoListArgs;

impl ReadRepoArgs {
    /// # Errors
    ///
    /// Returns an error if config loading/saving fails or if a provided path is not a git repository.
    pub async fn invoke(self) -> Result<()> {
        tokio::task::spawn_blocking(move || -> Result<()> {
            match self.command {
                ReadRepoCommand::Add(args) => {
                    let mut config = VanityConfig::load()?;
                    let canonical = config.add_read_repo(&args.path)?;
                    config.save()?;
                    println!("{}", canonical.display());
                }
                ReadRepoCommand::List(_) => {
                    let config = VanityConfig::load()?;
                    for repo in config.read_repos {
                        println!("{}", repo.display());
                    }
                }
            }
            Ok(())
        })
        .await
        .map_err(|err| eyre::eyre!("read-repo task failed: {err}"))?
    }
}

impl ToArgs for ReadRepoArgs {
    fn to_args(&self) -> Vec<OsString> {
        let mut args = Vec::new();
        match &self.command {
            ReadRepoCommand::Add(add) => {
                args.push("add".into());
                args.push(add.path.as_os_str().into());
            }
            ReadRepoCommand::List(_) => {
                args.push("list".into());
            }
        }
        args
    }
}

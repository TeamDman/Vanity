use crate::cli::ToArgs;
use crate::vanity::VanityConfig;
use arbitrary::Arbitrary;
use eyre::Result;
use eyre::bail;
use facet::Facet;
use figue as args;
use std::ffi::OsString;
use std::path::PathBuf;

#[derive(Facet, Arbitrary, Debug, PartialEq)]
pub struct ThisRepoArgs {
    #[facet(args::subcommand)]
    pub command: ThisRepoCommand,
}

#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[repr(u8)]
pub enum ThisRepoCommand {
    Set(ThisRepoSetArgs),
    Show(ThisRepoShowArgs),
}

#[derive(Facet, Arbitrary, Debug, PartialEq)]
pub struct ThisRepoSetArgs {
    #[facet(args::positional)]
    pub path: PathBuf,
}

#[derive(Facet, Arbitrary, Debug, PartialEq, Default)]
pub struct ThisRepoShowArgs;

impl ThisRepoArgs {
    /// # Errors
    ///
    /// Returns an error if config loading/saving fails or if a provided path is not a git repository.
    pub async fn invoke(self) -> Result<()> {
        tokio::task::spawn_blocking(move || -> Result<()> {
            match self.command {
                ThisRepoCommand::Set(args) => {
                    let mut config = VanityConfig::load()?;
                    let canonical = config.set_this_repo(&args.path)?;
                    config.save()?;
                    println!("{}", canonical.display());
                }
                ThisRepoCommand::Show(_) => {
                    let config = VanityConfig::load()?;
                    let Some(path) = config.this_repo else {
                        bail!("this-repo is not configured. Run: this-repo set <path>");
                    };
                    println!("{}", path.display());
                }
            }
            Ok(())
        })
        .await
        .map_err(|err| eyre::eyre!("this-repo task failed: {err}"))?
    }
}

impl ToArgs for ThisRepoArgs {
    fn to_args(&self) -> Vec<OsString> {
        let mut args = Vec::new();
        match &self.command {
            ThisRepoCommand::Set(set) => {
                args.push("set".into());
                args.push(set.path.as_os_str().into());
            }
            ThisRepoCommand::Show(_) => {
                args.push("show".into());
            }
        }
        args
    }
}

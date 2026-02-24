use crate::cli::ToArgs;
use arbitrary::Arbitrary;
use eyre::Result;
use facet::Facet;

/// Show the home path.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
pub struct HomePathShowArgs;

impl HomePathShowArgs {
    /// # Errors
    ///
    /// This function does not return any errors.
    #[expect(clippy::unused_async)]
    pub async fn invoke(self) -> Result<()> {
        println!("{}", crate::paths::APP_HOME.display());
        Ok(())
    }
}

impl ToArgs for HomePathShowArgs {}

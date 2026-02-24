use crate::cli::ToArgs;
use arbitrary::Arbitrary;
use eyre::Result;
use facet::Facet;

/// Show the cache path.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
pub struct CachePathShowArgs;

impl CachePathShowArgs {
    /// # Errors
    ///
    /// This function does not return any errors.
    #[expect(clippy::unused_async)]
    pub async fn invoke(self) -> Result<()> {
        println!("{}", crate::paths::CACHE_DIR.display());
        Ok(())
    }
}

impl ToArgs for CachePathShowArgs {}

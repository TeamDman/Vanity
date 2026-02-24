mod app_home;
mod cache;

pub use app_home::*;
pub use cache::*;

pub const APP_HOME_ENV_VAR: &str = "VANITY_HOME_DIR";
pub const APP_HOME_DIR_NAME: &str = "vanity";

pub const APP_CACHE_ENV_VAR: &str = "VANITY_CACHE_DIR";
pub const APP_CACHE_DIR_NAME: &str = "vanity";

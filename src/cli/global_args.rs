//! Global arguments that apply to all commands.

use crate::cli::ToArgs;
use crate::logging::LoggingConfig;
use arbitrary::Arbitrary;
use chrono::Local;
use facet::Facet;
use figue::{self as args};
use std::ffi::OsString;
use std::path::PathBuf;
use std::str::FromStr;
use tracing::level_filters::LevelFilter;

/// Global arguments that apply to all commands.
#[derive(Facet, Arbitrary, Debug, Default, PartialEq)]
pub struct GlobalArgs {
    /// Enable debug logging, including backtraces on panics.
    #[facet(args::named, default)]
    pub debug: bool,

    /// Log level filter directive.
    #[facet(args::named)]
    pub log_filter: Option<String>,

    /// Write structured ndjson logs to this file or directory.
    #[facet(args::named)]
    pub log_file: Option<PathBuf>,
}

impl GlobalArgs {
    /// Get the logging configuration from CLI arguments.
    ///
    /// # Errors
    ///
    /// This function will return an error if the log filter string is invalid.
    pub fn logging_config(&self) -> eyre::Result<LoggingConfig> {
        Ok(LoggingConfig {
            default_directive: match (self.debug, &self.log_filter) {
                (true, _) => LevelFilter::DEBUG,
                (false, Some(filter)) => LevelFilter::from_str(filter)?,
                (false, None) => LevelFilter::INFO,
            }
            .into(),
            json_log_path: match &self.log_file {
                None => None,
                Some(path) if path.is_dir() => {
                    let timestamp = Local::now().format("%Y-%m-%d_%H-%M-%S");
                    let filename = format!("log_{timestamp}.ndjson");
                    Some(path.join(filename))
                }
                Some(path) => Some(path.clone()),
            },
        })
    }
}

impl ToArgs for GlobalArgs {
    fn to_args(&self) -> Vec<OsString> {
        let mut args = Vec::new();
        if self.debug {
            args.push("--debug".into());
        }
        if let Some(filter) = &self.log_filter {
            args.push("--log-filter".into());
            args.push(filter.into());
        }
        if let Some(path) = &self.log_file {
            args.push("--log-file".into());
            args.push(path.as_os_str().into());
        }
        args
    }
}

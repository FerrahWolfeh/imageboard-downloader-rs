use std::io;

use ibdl_extractors::error::ExtractorError;
use thiserror::Error;

#[allow(clippy::enum_variant_names)]
#[derive(Error, Debug)]
pub enum CliError {
    #[error("Failed to authenticate to imageboard: {source}")]
    CoreAuthFail {
        #[from]
        source: ibdl_extractors::auth::Error,
    },

    #[error("Failed to authenticate to imageboard: {source}")]
    InternalExtractorAuthFail {
        #[from]
        source: ExtractorError,
    },

    #[error("Failed to write input to console: {source}")]
    DialoguerIOFail {
        #[from]
        source: dialoguer::Error,
    },

    #[error("Failed to access file: {source}")]
    IOError {
        #[from]
        source: io::Error,
    },

    #[error("Whatever you did, it definetly shouldn't happen...")]
    ImpossibleExecutionPath,

    #[error("This operation is currently unsupported for this imageboard")]
    ExtractorUnsupportedMode,

    #[error("Failed to read server config")]
    ServerConfigSerializeFail,

    #[error("Selected server does not exist.")]
    ServerNotExists,

    #[error("No posts given")]
    NoPostsInInput,

    #[error("This program has been compiled without {imageboard} support.")]
    ImageboardNotEnabled { imageboard: String },

    #[error(
        "No imageboards are enabled. Recompile the program with at least one imageboard feature enabled."
    )]
    NoImageboardsEnabled,

    /// Failed to deserialize the TOML content of the blacklist configuration file.
    /// Wraps an underlying `toml::de::Error`.
    #[error("Failed to decode blacklis: {source}")]
    BlacklistDecodeError {
        #[from]
        source: toml::de::Error,
    },
}

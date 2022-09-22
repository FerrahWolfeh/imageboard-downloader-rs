use ibdl_common::{auth::AuthError, reqwest, tokio};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ExtractorError {
    #[error("Too many tags, got: {current} while this imageboard only supports a max of {max}")]
    TooManyTags { current: usize, max: u64 },

    #[error("No posts found for tag selection")]
    ZeroPosts,

    #[error("Imageboard returned an invalid response")]
    InvalidServerResponse,

    #[error("Connection Error")]
    ConnectionError(#[from] reqwest::Error),

    #[error("Authentication failed. error: {source}")]
    AuthenticationFailure {
        #[from]
        source: AuthError,
    },

    #[error("Error while reading Global blacklist file. error: {source}")]
    BlacklistIOError {
        #[from]
        source: tokio::io::Error,
    },

    #[error("Failed to map posts")]
    PostMapFailure,

    #[error("Failed to decode blacklist.toml in {path}")]
    BlacklistDecodeError { path: String },

    #[error("Invalid imageboard selected for this extractor: {imgboard}")]
    InvalidImageboard { imgboard: String },

    #[error("Impossible execution path")]
    ImpossibleBehavior,
}

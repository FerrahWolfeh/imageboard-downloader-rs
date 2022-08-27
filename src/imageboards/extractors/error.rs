use thiserror::Error;

#[derive(Error, Debug)]
pub enum ExtractorError {
    #[error("Too many tags, got: {current} while this imageboard supports a max of {max}")]
    TooManyTags { current: usize, max: u64 },

    #[error("No posts found for tag selection")]
    ZeroPosts,

    #[error("Imageboard returned an invalid response")]
    InvalidServerResponse,

    #[error("Connection Error")]
    ConnectionError(#[from] reqwest::Error),
}

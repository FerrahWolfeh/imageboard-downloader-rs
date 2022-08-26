use thiserror::Error;

#[derive(Error, Debug)]
pub enum ExtractorError {
    #[error("Too many tags, got: {current} while this imageboard supports a max of {max}")]
    TooManyTagsError { current: usize, max: u64 },

    #[error("Invalid global blacklist file")]
    BlacklistParseError,
}

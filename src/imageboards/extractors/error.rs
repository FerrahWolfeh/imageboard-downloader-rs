use thiserror::Error;

#[derive(Error, Debug)]
pub enum ExtractorError {
    #[error("Invalid rating tag: {0}")]
    RatingParseError(String),

    #[error("Invalid global blacklist file")]
    BlacklistParseError,
}

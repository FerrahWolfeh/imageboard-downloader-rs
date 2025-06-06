use crate::auth::Error;
use ibdl_common::post::Post;
use thiserror::Error;
use tokio::sync::mpsc::error::SendError;

/// Enumerates the possible errors that can arise during extractor operations.
///
/// This error type consolidates issues from various stages of the extraction process,
/// including network requests, API interactions, data parsing, authentication,
/// and internal channel communication.
#[derive(Error, Debug)]
pub enum ExtractorError {
    /// Failed to send a `Post` through a synchronous (`std::sync::mpsc`) channel.
    /// This typically occurs in multi-threaded contexts where posts are passed between threads.
    #[error("Failed to send post through channel")]
    SyncChannelSendFail(#[from] std::sync::mpsc::SendError<Post>),

    /// Failed to send a `Post` through an asynchronous (`tokio::sync::mpsc`) channel.
    /// This is common in async tasks passing posts to other parts of the application.
    #[error("Failed to send post through channel")]
    ChannelSendFail(#[from] SendError<Post>),

    /// An attempt was made to fetch posts from page zero, which is invalid for most imageboard APIs.
    #[error("Page number cannot be zero.")]
    ZeroPage,

    /// The number of tags provided for a search query exceeds the limit supported by the target imageboard.
    /// `current` is the number of tags provided, and `max` is the imageboard's limit.
    #[error("Too many tags, got: {current} while this imageboard only supports a max of {max}")]
    TooManyTags { current: usize, max: u64 },

    /// The imageboard API returned no posts for the given search tags or query.
    #[error("No posts found for tag selection")]
    ZeroPosts,

    /// The imageboard server returned a response that could not be understood or was not in the expected format.
    #[error("Imageboard returned an invalid response")]
    InvalidServerResponse,

    /// An error occurred during a network request (e.g., connection timeout, DNS resolution failure).
    /// Wraps an underlying `reqwest::Error`.
    #[error("Connection Error")]
    ConnectionError(#[from] reqwest::Error),

    /// Authentication with the imageboard server failed.
    /// Wraps an `auth::Error` which provides more specific details about the authentication failure.
    #[error("Authentication failed. error: {source}")]
    AuthenticationFailure {
        #[from]
        source: Error,
    },

    /// An error occurred while deserializing a JSON response from the imageboard API.
    /// Wraps an underlying `serde_json::Error`.
    #[error("Error while deserializing JSON")]
    JsonSerializeFail(#[from] serde_json::Error),

    /// An error occurred during the process of mapping raw API data to `Post` structs.
    /// This might indicate unexpected data structures or missing essential fields.
    #[error("Failed to map posts")]
    PostMapFailure,

    /// An attempt was made to use an extractor with an imageboard type it does not support.
    /// `imgboard` contains the name or identifier of the incompatible imageboard.
    #[error("Invalid imageboard selected for this extractor: {imgboard}")]
    InvalidImageboard { imgboard: String },

    /// Indicates a state or condition that should be logically impossible to reach,
    /// often signifying a bug in the extractor's internal logic.
    #[error("Impossible execution path")]
    ImpossibleBehavior,

    /// The requested operation is not supported by the current imageboard server or its API.
    #[error("Unsupported operation for this server")]
    UnsupportedOperation,

    /// A numeric conversion failed, typically when trying to cast a larger integer type to a smaller one
    /// and the value is out of range. Wraps `std::num::TryFromIntError`.
    #[error("Integer number is too high to be casted")]
    IntConversionFail {
        #[from]
        source: std::num::TryFromIntError,
    },

    /// Failed to send length data (e.g., total number of posts) to a progress counter,
    /// likely via an asynchronous channel. Wraps `tokio::sync::mpsc::error::SendError<u64>`.
    #[error("Error sending length data to progress counter: {source}")]
    SendLengthFail {
        #[from]
        source: SendError<u64>,
    },

    /// A `Post` object is missing an essential field that is required for its proper function or for downloading.
    /// `field` indicates the name of the missing field.
    #[error("Post is missing an essential field {field}")]
    MissingField { field: String },
}

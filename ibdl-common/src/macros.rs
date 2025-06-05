//! # Utility Macros
//!
//! This module provides a collection of utility macros used throughout the
//! `imageboard-downloader` ecosystem. These macros are designed to reduce
//! boilerplate code and simplify common operations.

/// Creates a `reqwest::Client` with a specific user-agent.
///
/// This macro expects an expression `$x` that has a field named `client_user_agent`.
/// The value of this field will be used as the User-Agent string for the client.
///
/// # Panics
/// This macro will panic if the `reqwest::ClientBuilder::build()` method fails.
///
/// # Example
/// ```rust
/// // Assuming a struct `Config` with a `client_user_agent` field
/// struct Config { client_user_agent: String }
/// let config = Config { client_user_agent: "MyAwesomeApp/1.0".to_string() };
/// let client = ibdl_common::client!(config);
/// ```
#[macro_export]
macro_rules! client {
    ($x:expr) => {{
        $crate::reqwest::Client::builder()
            .user_agent(&$x.client_user_agent)
            .build()
            .unwrap()
    }};
}

/// Creates a `reqwest::Client` with a user-agent obtained from a method call.
///
/// This macro expects an expression `$x` that has a method `user_agent()`
/// which returns a type that can be used as a User-Agent string (e.g., `&str` or `String`).
///
/// # Panics
/// This macro will panic if the `reqwest::ClientBuilder::build()` method fails.
///
/// # Example
/// ```rust
/// // Assuming a struct `Downloader` with a `user_agent()` method
/// struct Downloader { /* ... */ }
/// impl Downloader { fn user_agent(&self) -> &str { "MyDownloader/1.0" } }
/// let downloader = Downloader { /* ... */ };
/// let client = ibdl_common::client_imgb!(downloader);
/// ```
#[macro_export]
macro_rules! client_imgb {
    ($x:expr) => {{
        $crate::reqwest::Client::builder()
            .user_agent($x.user_agent())
            .build()
            .unwrap()
    }};
}

/// Joins a slice or `Vec` of tags (strings) into a single `String`, separated by `+`.
///
/// This is commonly used for formatting tags for imageboard API search queries.
///
/// # Example
/// ```rust
/// let tags = vec!["tag1".to_string(), "tag2".to_string(), "tag3".to_string()];
/// let joined = ibdl_common::join_tags!(tags);
/// assert_eq!(joined, "tag1+tag2+tag3");
/// ```
#[macro_export]
macro_rules! join_tags {
    ($x:expr) => {{
        let tl = $x.join("+");
        tl
    }};
}

/// Extracts the file extension from a URL string.
///
/// It splits the URL by `.` and takes the last part.
///
/// # Panics
/// This macro will panic if the input string `$x` does not contain a `.` character,
/// or if the string is empty after the last `.`.
///
/// # Example
/// ```rust
/// let url = "https://example.com/image.png";
/// let ext = ibdl_common::extract_ext_from_url!(url);
/// assert_eq!(ext, "png");
/// ```
#[macro_export]
macro_rules! extract_ext_from_url {
    ($x:expr) => {{
        let ext = $x.split('.').next_back().unwrap();
        ext.to_string()
    }};
}

/// Returns a static slice containing all variants of the `Rating` enum.
///
/// Useful for iterating over all possible ratings or for default configurations.
/// Requires `ibdl_common::post::rating::Rating` to be in scope.
#[macro_export]
macro_rules! all_ratings {
    () => {
        &[
            Rating::Safe,
            Rating::Questionable,
            Rating::Explicit,
            Rating::Unknown, // Assuming Rating is from crate::post::rating
        ]
    };
}

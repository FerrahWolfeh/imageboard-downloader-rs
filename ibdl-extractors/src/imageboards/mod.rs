//! Imageboard-specific extractor modules.
//! # Extractors
//!
//! This module contains submodules, each dedicated to a specific imageboard or type of imageboard API.
//! These submodules implement the `Extractor` trait (if applicable, assuming this trait exists at `crate::extractor::Extractor`)
//! to provide a standardized way of fetching post information.
//!
//! Extractors connect to an imageboard's API, search for posts based on provided tags,
//! and parse the results into a `PostQueue`, which is a collection
//! of `Post` items.
//!
//! ## General example
//! Most extractors have a common set of public methods that should be the default way of implementing and interacting with them.
//!
//! ### Example with the `Danbooru` extractor
//!
//! ```rust
//! # #[cfg(feature = "danbooru")]
//! # async fn danbooru_example() -> Result<(), Box<dyn std::error::Error>> {
//! use ibdl_extractors::prelude::*; // Assumes prelude exports extractors and ImageBoards
//!
//! let tags = ["umbreon", "espeon"]; // The tags to search
//!
//! // Initialize the extractor for Danbooru
//! let mut extractor = DanbooruExtractor::new(&tags, false, false);
//!
//! // Attempt non-interactive authentication (e.g., using saved credentials)
//! // For public posts, auth might not be strictly necessary or might proceed anonymously.
//! extractor.auth(false).await?;
//!
//! // Search for posts, starting from page 1, with a limit of 1 post.
//! let posts = extractor.full_search(Some(1), Some(1)).await?;
//!
//! println!("Found {} posts.", posts.posts.len());
//! if let Some(post) = posts.posts.first() {
//!     println!("First post ID: {}", post.id);
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ### Example with the `Gelbooru` extractor
//!
//! The Gelbooru extractor supports multiple websites, so to use it correctly, an additional option needs to be set.
//!
//! ```rust
//! # #[cfg(feature = "gelbooru")]
//! # async fn gelbooru_example() -> Result<(), Box<dyn std::error::Error>> {
//! use ibdl_extractors::prelude::*; // Assumes prelude exports extractors and ImageBoards
//! use ibdl_common::ImageBoards;    // Use this if ImageBoards is not in prelude
//!
//!     let tags = ["umbreon", "espeon"]; // The tags to search
//!
//!     // Initialize the extractor for Gelbooru
//!     let mut extractor = GelbooruExtractor::new(&tags, false, false)
//!         .set_imageboard(ImageBoards::Gelbooru)?; // Set to a specific Gelbooru-compatible site
//!
//!     // Attempt non-interactive authentication
//!     extractor.auth(false).await?;
//!
//!     // Search for posts
//!     let posts = extractor.full_search(Some(1), Some(1)).await?;
//!
//!     println!("Found {} posts.", posts.posts.len());
//!     if let Some(post) = posts.posts.first() {
//!         println!("First post ID: {}", post.id);
//!     }
//! # Ok(())
//! # }
//! # #[cfg(feature = "gelbooru")]
//! # tokio_test::block_on(gelbooru_example());
//!```
//!
//!
#![deny(clippy::nursery)]

/// Extractor for Danbooru and Danbooru-like imageboards (e.g., Safebooru).
#[cfg(feature = "danbooru")]
pub mod danbooru;

/// Extractor for e621 and e621-like imageboards (e.g., e926).
#[cfg(feature = "e621")]
pub mod e621;

/// Extractor for Gelbooru and Gelbooru-like imageboards (e.g., Rule34, Gelbooru 0.2.x).
#[cfg(feature = "gelbooru")]
pub mod gelbooru;

/// Extractor for Moebooru-based imageboards (e.g., Konachan, Yande.re).
#[cfg(feature = "moebooru")]
pub mod moebooru;

/// A prelude module for conveniently importing common extractor types, traits, and enums.
pub mod prelude;

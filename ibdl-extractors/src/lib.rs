#![deny(clippy::nursery, clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::struct_field_names)]
//! # IBDL Extractors
//!
//! `ibdl-extractors` is a crate designed to provide a robust and standardized way to
//! interact with various imageboard APIs.
//!
//! It handles the complexities of fetching
//! post data, managing authentication, and applying blacklists, allowing developers
//! to easily integrate imageboard content into their applications.
//!
//! ## Core Features
//!
//! - **Multiple Imageboard Support**: Built-in support for popular imageboards like
//!   Danbooru, e621, Gelbooru, and Moebooru, with an extensible architecture.
//! - **Authentication**: Handles API key and username authentication for sites that require it.
//! - **Blacklisting**: Supports global and site-specific tag blacklisting to filter content.
//! - **Configuration**: Manages server-specific configurations like API endpoints and rate limits.
//!
//! ## Quick Start Example
//!
//! ```rust
//! use ibdl_extractors::prelude::*; // Essential traits and types
//! use ibdl_extractors::imageboards::danbooru::DanbooruExtractor; // Specific extractor
//!
//! // Define the tags to search for
//! let tags = ["cat_ears", "solo"];
//!
//! // Initialize the Danbooru extractor
//! // Parameters: tags, include_safe_posts (true/false), disable_blacklist (true/false)
//! let mut extractor = DanbooruExtractor::new(&tags, true, false);
//!
//! // Attempt non-interactive authentication (e.g., using cached credentials or anonymous access)
//! // For a real application, you might prompt the user if `auth(true).await` is used.
//! if let Err(e) = extractor.auth(false).await {
//!     eprintln!("Authentication failed or was not supported: {}", e);
//!     // Decide if to proceed or handle the error
//! }
//!
//! // Perform a search for posts
//! // Parameters: starting_page (Option<u16>), post_limit (Option<u16>)
//! match extractor.full_search(Some(1), Some(5)).await {
//!     Ok(post_queue) => {
//!         println!("Found {} posts for tags: {:?}", post_queue.posts.len(), tags);
//!         for post in post_queue.posts {
//!             println!("Post ID: {}, URL: {}", post.id, post.url);
//!         }
//!     }
//!     Err(e) => {
//!         eprintln!("Error during search: {}", e);
//!     }
//! }
//! ```
extern crate ibdl_common;

/// Handles user authentication and configuration for imageboard websites.
pub mod auth;
/// Manages global and site-specific tag blacklisting to filter posts.
pub mod blacklist;
/// Defines error types specific to the extractor operations.
pub mod error;
/// Contains the core `Extractor` trait and related structures for API interaction.
pub mod extractor;
/// Manages server-specific configurations, including API endpoints and default settings.
pub mod extractor_config;
/// Provides concrete implementations of extractors for various imageboard sites.
pub mod imageboards;
/// A prelude module for conveniently importing common types and traits from this crate.
pub mod prelude;

//! Modules that work by parsing post info from a imageboard API into a list of [Posts](ibdl_common::post).
//! # Extractors
//!
//! All modules implementing [`Extractor`] work by connecting to a imageboard website, searching for posts with the tags supplied and parsing all of them into a [`PostQueue`](ibdl_common::post::PostQueue).
//!
//! ## General example
//! Most extractors have a common set of public methods that should be the default way of implementing and interacting with them.
//!
//! ### Example with the `Danbooru` extractor
//! ```rust
//! use imageboard_downloader::*;
//!
//! async fn test() {
//!     let tags = ["umbreon", "espeon"]; // The tags to search
//!     
//!     let safe_mode = false; // Setting this to true, will ignore searching NSFW posts
//!
//!     let disable_blacklist = false; // Will filter all items according to what's set in GBL
//!
//!     let mut unit = DanbooruExtractor::new(&tags, safe_mode, disable_blacklist); // Initialize
//!
//!     let prompt = true; // If true, will ask the user to input thei username and API key.
//!
//!     unit.auth(prompt).await.unwrap(); // Try to authenticate
//!
//!     let start_page = Some(1); // Start searching from the first page
//!
//!     let limit = Some(50); // Max number of posts to download
//!
//!     let posts = unit.full_search(start_page, limit).await.unwrap(); // and then, finally search
//!
//!     println!("{:#?}", posts.posts);
//! }
//!```
//!
//! ### Example with the `Gelbooru` extractor
//!
//! The Gelbooru extractor supports multiple websites, so to use it correctly, an additional option needs to be set.
//!
//! ```rust
//! use imageboard_downloader::*;
//!
//! async fn test() {
//!     let tags = ["umbreon", "espeon"]; // The tags to search
//!     
//!     let safe_mode = false; // Setting this to true, will ignore searching NSFW posts
//!
//!     let disable_blacklist = false; // Will filter all items according to what's set in GBL
//!
//!     let mut unit = GelbooruExtractor::new(&tags, safe_mode, disable_blacklist)
//!         .set_imageboard(ImageBoards::Rule34).expect("Invalid imageboard"); // Here the imageboard was set to Rule34
//!
//!
//!     let prompt = true; // If true, will ask the user to input thei username and API key.
//!
//!     let start_page = Some(1); // Start searching from the first page
//!
//!     let limit = Some(50); // Max number of posts to download
//!
//!     let posts = unit.full_search(start_page, limit).await.unwrap(); // and then, finally search
//!
//!     println!("{:#?}", posts.posts);
//! }
//!```
//!
//!
#![deny(clippy::nursery)]

pub mod danbooru;

pub mod e621;

pub mod gelbooru;

pub mod moebooru;

pub mod prelude;

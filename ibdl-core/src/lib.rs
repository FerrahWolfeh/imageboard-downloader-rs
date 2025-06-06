//! # IBDL-CORE
//!
//! This crate provides the core downloading infrastructure for the `imageboard-downloader` project.
//! It is responsible for managing the asynchronous download queue, handling the actual
//! download and saving of files (either to individual folders or into CBZ archives),
//! reporting progress, and defining common error types related to the download process.
//!
//! ## Core Features
//!
//! - **Asynchronous Download Queue**: Efficiently manages multiple concurrent downloads using Tokio.
//! - **Flexible Output**: Supports saving files to a structured directory or packaging them into `.cbz` archives (if the `cbz` feature is enabled).
//! - **Progress Reporting**: Provides a flexible trait-based system for progress updates,
//!   allowing integration with various UI implementations (e.g., `indicatif` in `ibdl-cli`).
//! - **Error Handling**: Defines a clear set of `DownloaderError` types for download-related issues.
//!
//! ## Key Modules
//!
//! - [`async_queue`]: Contains the [`Queue`](crate::async_queue::Queue) struct, the primary interface for initiating and managing downloads.
//! - [`error`]: Defines [`DownloaderError`](crate::error::DownloaderError), the main error type for operations within this crate.
//! - [`progress`]: Exposes traits like [`ProgressListener`](crate::progress::ProgressListener) and [`DownloadProgressUpdater`](crate::progress::DownloadProgressUpdater)
//!   for detailed progress tracking of the download process.
//!
//! ## Example Usage
//!
//! The primary component for users of this crate is the `Queue`.
//! Here's a conceptual example of how it might be used:
//!
//! ```rust
//! use ibdl_core::async_queue::{Queue, QueueOpts};
//! use ibdl_core::progress::no_op_progress_listener; // For example purposes
//! use ibdl_common::post::{Post, NameType};
//! use ibdl_common::ImageBoards; // Assuming Post, NameType, etc. are available
//! use ibdl_extractors::extractor_config::ServerConfig; // For ServerConfig
//! use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};
//! use std::path::PathBuf;
//!
//! async fn download_example_posts(
//!     posts_rx: UnboundedReceiver<Post>, // Channel receiving Post objects from an extractor
//!     server_conf: ServerConfig,
//!     output_dir: PathBuf,
//! ) -> Result<u64, ibdl_core::error::DownloaderError> {
//!     let queue_opts = QueueOpts {
//!         save_as_cbz: false, // Save to folder
//!         pool_download: false,
//!         name_type: NameType::ID,
//!         annotate: false,
//!     };
//!
//!     let progress_listener = no_op_progress_listener(); // Or a real progress listener
//!
//!     let queue = Queue::new(
//!         server_conf, 5, None, queue_opts, Some(progress_listener.clone()),
//!     );
//!
//!     let download_handle = queue.setup_async_downloader(output_dir, posts_rx);
//!     let total_downloaded = download_handle.await.unwrap()?; // Handle JoinError and DownloaderError
//!     Ok(total_downloaded)
//! }
//! ```
//!
//! The `generate_output_path` function is also provided as a utility for determining
//! conventional output paths based on imageboard, tags, and output mode.
#![deny(clippy::nursery)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::struct_field_names)]
pub use clap;
use ibdl_common::ImageBoards;
use std::path::{Path, PathBuf};

/// Manages the asynchronous download queue, file saving, and related logic.
pub mod async_queue;
/// Defines error types for the download process.
pub mod error;
/// Provides traits and utilities for download progress reporting.
pub mod progress;

/// Generates a standardized output path for downloaded content.
///
/// This function constructs a `PathBuf` based on the main output directory,
/// the imageboard, tags, download mode (CBZ or folder), and an optional pool ID.
/// The goal is to create a consistent and descriptive directory structure or filename.
///
/// # Path Construction Logic:
///
/// The final path is `{main_path}/{imageboard_name}/{path_stem}{.cbz_if_applicable}`.
///
/// 1.  **`imageboard_name`**: The string representation of the `imageboard` enum variant (e.g., "Danbooru").
///
/// 2.  **`path_stem`** (used as subdirectory name or CBZ filename stem):
///     *   Let `joined_tags = tags.join(" ")`.
///     *   If `joined_tags` contains "fav:": `path_stem` is "Favorites".
///     *   Else if compiling for Windows: `path_stem` is `joined_tags.replace(':', "_")`.
///         (Note: In this case, `pool_id` is not directly used to form the stem if `tags` are non-empty and don't contain "fav:").
///     *   Else (not "fav:" and not Windows), if `pool_id` is `Some(id)`: `path_stem` is `id.to_string()`.
///     *   Else (not "fav:", not Windows, and `pool_id` is `None`): `path_stem` is `joined_tags`.
///
///     *Important Note on `pool_id` behavior*: If `tags` is an empty slice (common for pool downloads)
///     and the compilation target is Windows, the `path_stem` will currently result in an empty string
///     due to the `cfg!(windows)` check taking precedence over the `pool_id` check in that branch.
///     On non-Windows systems, an empty `tags` slice with a `pool_id` will correctly use the pool ID as the stem.
///
/// 3.  **Final Path**:
///     *   If `cbz_mode` is `true`: The path is `main_path.join(imageboard_name).join(path_stem + ".cbz")`.
///     *   If `cbz_mode` is `false`: The path is `main_path.join(imageboard_name).join(path_stem)`.
///
/// # Arguments
///
/// * `main_path`: The root directory where downloads should be saved or where the imageboard-specific subdirectory will be created.
/// * `imageboard`: The `ImageBoards` enum variant specifying the source.
/// * `tags`: A slice of strings representing the tags used for the download.
///   Special handling for "fav:" tag. If downloading a pool, this is often empty.
/// * `cbz_mode`: A boolean indicating if the output path should point to a CBZ file (`true`)
///   or a directory (`false`).
/// * `pool_id`: An optional `u32` pool ID. Used to form `path_stem` under certain conditions (see logic above).
///
/// # Returns
///
/// A `PathBuf` representing the generated output path.
///
/// # Examples
///
/// ```
/// use ibdl_core::generate_output_path;
/// use ibdl_common::ImageBoards;
/// use std::path::Path;
///
/// let output_root = Path::new("/downloads");
///
/// // Example 1: Tag search, folder mode
/// let tags1 = &[String::from("cat_ears"), String::from("solo")];
/// let path1 = generate_output_path(output_root, ImageBoards::Danbooru, tags1, false, None);
/// // Expected on non-Windows: /downloads/Danbooru/cat_ears solo
/// // Expected on Windows: /downloads/Danbooru/cat_ears solo
/// println!("Path 1 (actual): {}", path1.display());
///
/// // Example 2: Pool download, CBZ mode (non-Windows)
/// let empty_tags = &[];
/// if !cfg!(windows) {
///     let path2_non_windows = generate_output_path(output_root, ImageBoards::E621, empty_tags, true, Some(12345));
///     assert_eq!(path2_non_windows, Path::new("/downloads/e621/12345.cbz"));
/// }
///
/// // Example 3: Favorites, folder mode
/// let fav_tags = &[String::from("fav:user123")];
/// let path3 = generate_output_path(output_root, ImageBoards::Danbooru, fav_tags, false, None);
/// assert_eq!(path3, Path::new("/downloads/Danbooru/Favorites"));
///
/// // Example 4: Pool download, folder mode (Windows - illustrates potential issue)
/// if cfg!(windows) {
///     let path4_windows = generate_output_path(output_root, ImageBoards::Gelbooru, empty_tags, false, Some(54321));
///     assert_eq!(path4_windows, Path::new("/downloads/Gelbooru/")); // Stem is empty
/// }
/// ```
#[inline]
pub fn generate_output_path(
    main_path: &Path,
    imageboard: ImageBoards,
    tags: &[String],
    cbz_mode: bool,
    pool_id: Option<u32>,
) -> PathBuf {
    let tag_string = tags.join(" ");
    let tag_path_string = if tag_string.contains("fav:") {
        String::from("Favorites")
    } else if cfg!(windows) {
        tag_string.replace(':', "_")
    } else if let Some(id) = pool_id {
        id.to_string()
    } else {
        tag_string
    };

    let pbuf = main_path.join(Path::new(&imageboard.to_string()));

    if cbz_mode {
        return pbuf.join(Path::new(&format!("{}.cbz", tag_path_string)));
    }
    pbuf.join(Path::new(&tag_path_string))
}

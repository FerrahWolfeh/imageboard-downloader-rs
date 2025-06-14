use std::fmt::Debug;
use std::sync::Arc;

/// Trait for reporting overall progress, typically for a collection of items (e.g., posts).
/// All methods should be thread-safe.
pub trait ProgressListener: Send + Sync + Debug {
    /// Sets the total number of items for the main progress.
    fn set_main_total(&self, total: u64);
    /// Increments the total number of items for the main progress.
    /// Called when more items are discovered (e.g., during pagination).
    fn inc_main_total(&self, delta: u64);
    /// Signals that one main item has been processed successfully.
    fn main_tick(&self);
    /// Signals that `delta` main items have been processed successfully.
    fn main_inc_by(&self, delta: u64);
    /// Signals that all main item processing is complete.
    fn main_done(&self);

    /// Adds a new task for individual download progress tracking (e.g., a single file).
    ///
    /// # Arguments
    /// * `name`: A descriptive name for the task (e.g., filename).
    /// * `total_size`: The total size in bytes of the item to be downloaded, if known.
    ///
    /// # Returns
    /// A `Box<dyn DownloadProgressUpdater>` to update the progress of this specific task.
    /// The core library will call methods on this updater.
    fn add_download_task(
        &self,
        name: String,
        total_size: Option<u64>,
    ) -> Box<dyn DownloadProgressUpdater>;

    /// Logs a message indicating a file/post was skipped and why.
    /// This message is intended for user visibility (e.g., printed above progress bars).
    fn log_skip_message(&self, file_name: &str, reason: &str);
}

/// Trait for updating the progress of an individual download task.
/// Implementations will typically wrap a specific progress bar or UI element.
pub trait DownloadProgressUpdater: Send + Sync + Debug {
    /// Sets the current number of bytes downloaded for this task.
    fn set_progress(&self, bytes_downloaded: u64);
    /// Sets or updates the total size of the item being downloaded.
    /// Useful if the size wasn't known at task creation or changes (e.g. Content-Length header received).
    fn set_total_size(&self, total_size: u64);
    /// Signals that this download task is finished (successfully or not).
    fn finish(&self);
    // Optional: Consider adding a method for error messages or other status updates.
    // fn set_message(&self, msg: String);
}

/// A no-operation implementation of `ProgressListener`.
/// Used as a default when no actual progress reporting is needed by the library consumer.
#[derive(Debug, Clone)]
pub struct NoOpProgressListener;

impl ProgressListener for NoOpProgressListener {
    fn set_main_total(&self, _total: u64) {}
    fn inc_main_total(&self, _delta: u64) {}
    fn main_tick(&self) {}
    fn main_inc_by(&self, _delta: u64) {}
    fn main_done(&self) {}
    fn add_download_task(
        &self,
        _name: String,
        _total_size: Option<u64>,
    ) -> Box<dyn DownloadProgressUpdater> {
        Box::new(NoOpDownloadProgressUpdater)
    }
    fn log_skip_message(&self, _file_name: &str, _reason: &str) {}
}

/// A no-operation implementation of `DownloadProgressUpdater`.
#[derive(Debug, Clone)]
pub struct NoOpDownloadProgressUpdater;

impl DownloadProgressUpdater for NoOpDownloadProgressUpdater {
    fn set_progress(&self, _bytes_downloaded: u64) {}
    fn set_total_size(&self, _total_size: u64) {}
    fn finish(&self) {}
}

/// Convenience type alias for a shared, thread-safe progress listener.
pub type SharedProgressListener = Arc<dyn ProgressListener>;

/// Returns a shared instance of a `NoOpProgressListener`.
pub fn no_op_progress_listener() -> SharedProgressListener {
    Arc::new(NoOpProgressListener)
}

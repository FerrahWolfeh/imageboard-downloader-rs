use ibdl_common::ImageBoards;
use indicatif::{
    HumanBytes, MultiProgress, ProgressBar, ProgressDrawTarget, ProgressState, ProgressStyle,
};
use std::{
    fmt::Write,
    sync::{
        atomic::{AtomicU64, AtomicUsize},
        Arc,
    },
    time::Duration,
};

const PROGRESS_CHARS: &str = "━━";

struct BarTemplates {
    pub main: &'static str,
    pub download: &'static str,
}

impl BarTemplates {
    /// Returns special-themed progress bar templates for each variant
    #[inline]
    pub fn new(imageboard: ImageBoards) -> Self {
        match imageboard {
            ImageBoards::E621 => BarTemplates {
                main: "{spinner:.yellow.bold} {elapsed_precise:.bold} {wide_bar:.blue/white.dim} {percent:.bold}  {pos:.yellow} (eta. {eta})",
                download: "{spinner:.blue.bold} {bar:40.yellow/white.dim} {percent:.bold} | {byte_progress:21.blue} @ {bytes_per_sec:>13.yellow} (eta. {eta:<4.blue})",
            },
            ImageBoards::Realbooru => BarTemplates {
                main: "{spinner:.red.bold} {elapsed_precise:.bold} {wide_bar:.red/white.dim} {percent:.bold}  {pos:.bold} (eta. {eta})", 
                download: "{spinner:.red.bold} {bar:40.red/white.dim} {percent:.bold} | {byte_progress:21.bold.green} @ {bytes_per_sec:>13.red} (eta. {eta:<4})",
            },
            _ => BarTemplates::default(),
        }
    }
}

impl Default for BarTemplates {
    fn default() -> Self {
        Self {
            main: "{spinner:.green.bold} {elapsed_precise:.bold} {wide_bar:.green/white.dim} {percent:.bold}  {pos:.green} (eta. {eta:.blue})",
            download: "{spinner:.green.bold} {bar:40.green/white.dim} {percent:.bold} | {byte_progress:21.green} @ {bytes_per_sec:>13.red} (eta. {eta:<4.blue})",
        }
    }
}

/// Struct to condense a commonly used duo of progress bar instances and counters for downloaded posts.
///
/// The main usage for this is to pass references of the counters across multiple threads while downloading.
#[derive(Clone)]
pub struct ProgressCounter {
    pub total_mtx: Arc<AtomicUsize>,
    pub downloaded_mtx: Arc<AtomicU64>,
    pub main: Arc<ProgressBar>,
    pub multi: Arc<MultiProgress>,
}

impl ProgressCounter {
    /// Initialize the main progress bar and the stat counters.
    ///
    /// The style that the main progress bar will use is based on the predefined styles for each variant of the ['ImageBoards' enum](ibdl_common::ImageBoards)
    pub fn initialize(len: u64, imageboard: ImageBoards) -> Arc<Self> {
        let template = BarTemplates::new(imageboard);
        let bar = ProgressBar::new(len).with_style(master_progress_style(&template));
        bar.set_draw_target(ProgressDrawTarget::stderr());
        bar.enable_steady_tick(Duration::from_millis(100));

        // Initialize the bars
        let multi = Arc::new(MultiProgress::new());
        let main = Arc::new(multi.add(bar));

        Arc::new(Self {
            main,
            multi,
            total_mtx: Default::default(),
            downloaded_mtx: Default::default(),
        })
    }

    /// About the same as `initialize`, but accepts predefined styles instead.
    pub fn initialize_custom_style(len: u64, style: ProgressStyle) -> Arc<Self> {
        let bar = ProgressBar::new(len).with_style(style);
        bar.set_draw_target(ProgressDrawTarget::stderr_with_hz(60));
        bar.enable_steady_tick(Duration::from_millis(100));

        // Initialize the bars
        let multi = Arc::new(MultiProgress::new());
        let main = Arc::new(multi.add(bar));

        Arc::new(Self {
            main,
            multi,
            total_mtx: Default::default(),
            downloaded_mtx: Default::default(),
        })
    }

    /// Adds a download bar under the main progress bar. Will use the predefined style present in the ['ImageBoards' enum](ibdl_common::ImageBoards)
    pub fn add_download_bar(&self, len: u64, imageboard: ImageBoards) -> ProgressBar {
        let template = BarTemplates::new(imageboard);
        let bar = ProgressBar::new(len).with_style(download_progress_style(&template));
        bar.set_draw_target(ProgressDrawTarget::stderr());

        self.multi.add(bar)
    }

    /// Adds a download bar under the main progress bar, with a custom style.
    pub fn add_download_custom_style(&self, len: u64, style: ProgressStyle) -> ProgressBar {
        let bar = ProgressBar::new(len).with_style(style);
        bar.set_draw_target(ProgressDrawTarget::stderr());

        self.multi.add(bar)
    }
}

fn master_progress_style(templates: &BarTemplates) -> ProgressStyle {
    ProgressStyle::default_bar()
        .template(templates.main)
        .unwrap()
        .with_key("pos", |state: &ProgressState, w: &mut dyn Write| {
            write!(w, "{}/{}", state.pos(), state.len().unwrap()).unwrap();
        })
        .with_key("percent", |state: &ProgressState, w: &mut dyn Write| {
            write!(w, "{:>3.0}%", state.fraction() * 100_f32).unwrap();
        })
        .with_key(
            "files_sec",
            |state: &ProgressState, w: &mut dyn Write| match state.per_sec() {
                files_sec if files_sec.abs() < f64::EPSILON => write!(w, "0 files/s").unwrap(),
                files_sec if files_sec < 1.0 => write!(w, "{:.2} s/file", 1.0 / files_sec).unwrap(),
                files_sec => write!(w, "{:.2} files/s", files_sec).unwrap(),
            },
        )
        .progress_chars(PROGRESS_CHARS)
}

fn download_progress_style(templates: &BarTemplates) -> ProgressStyle {
    ProgressStyle::default_bar()
        .template(templates.download)
        .unwrap()
        .with_key("percent", |state: &ProgressState, w: &mut dyn Write| {
            write!(w, "{:>3.0}%", state.fraction() * 100_f32).unwrap();
        })
        .with_key(
            "byte_progress",
            |state: &ProgressState, w: &mut dyn Write| {
                write!(
                    w,
                    "{}/{}",
                    HumanBytes(state.pos()),
                    HumanBytes(state.len().unwrap())
                )
                .unwrap();
            },
        )
        .progress_chars(PROGRESS_CHARS)
}

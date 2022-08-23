use indicatif::{
    HumanBytes, MultiProgress, ProgressBar, ProgressDrawTarget, ProgressState, ProgressStyle,
};
use std::{fmt::Write, sync::Arc, time::Duration};

use crate::ImageBoards;

const PROGRESS_CHARS: &str = "━━";

pub struct BarTemplates {
    pub main: &'static str,
    pub download: &'static str,
}

impl Default for BarTemplates {
    fn default() -> Self {
        Self {
            main: "{spinner:.green.bold} {elapsed_precise:.bold} {wide_bar:.green/white.dim} {percent:.bold}  {pos:.green} ({msg:.bold.blue} | eta. {eta:.blue})",
            download: "{spinner:.green.bold} {bar:40.green/white.dim} {percent:.bold} | {byte_progress:.green} @ {bytes_per_sec:>13.red} (eta. {eta:.blue})",
        }
    }
}

/// Struct to condense a commonly used duo of progress bar instances.
///
/// The main usage for this is to pass references of the progress bars across multiple threads while downloading.
pub struct ProgressArcs {
    pub main: Arc<ProgressBar>,
    pub multi: Arc<MultiProgress>,
}

impl ProgressArcs {
    pub fn initialize(len: u64, imageboard: ImageBoards) -> Arc<Self> {
        let bar = ProgressBar::new(len)
            .with_style(master_progress_style(&imageboard.progress_template()));
        bar.set_draw_target(ProgressDrawTarget::stderr_with_hz(60));
        bar.enable_steady_tick(Duration::from_millis(100));

        // Initialize the bars
        let multi = Arc::new(MultiProgress::new());
        let main = Arc::new(multi.add(bar));

        Arc::new(Self { main, multi })
    }
}

pub fn master_progress_style(templates: &BarTemplates) -> ProgressStyle {
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

pub fn download_progress_style(templates: &BarTemplates) -> ProgressStyle {
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

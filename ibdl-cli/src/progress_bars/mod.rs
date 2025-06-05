use ibdl_common::ImageBoards;
// Import the progress traits from ibdl_core
use ibdl_core::progress::{DownloadProgressUpdater, LogType, ProgressListener};
use indicatif::{
    HumanBytes, MultiProgress, ProgressBar, ProgressDrawTarget, ProgressState, ProgressStyle,
};
use owo_colors::OwoColorize;
use std::{fmt::Write, time::Duration};

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
            ImageBoards::E621 => Self {
                main: "{spinner:.yellow.bold} {elapsed_precise:.bold} {wide_bar:.blue/white.dim} {percent:.bold}  {pos:.yellow} (eta. {eta})",
                download: "{spinner:.blue.bold} {bar:40.yellow/white.dim} {percent:.bold} | {byte_progress:21.blue} @ {bytes_per_sec:>13.yellow} (eta. {eta:<4.blue})",
            },
            ImageBoards::GelbooruV0_2 => Self {
                main: "{spinner:.red.bold} {elapsed_precise:.bold} {wide_bar:.red/white.dim} {percent:.bold}  {pos:.bold} (eta. {eta})",
                download: "{spinner:.red.bold} {bar:40.red/white.dim} {percent:.bold} | {byte_progress:21.bold.green} @ {bytes_per_sec:>13.red} (eta. {eta:<4})",
            },
            _ => Self::default(),
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

/// Handles CLI progress display using `indicatif`.
///
/// This struct implements the `ProgressListener` trait from `ibdl_core`.
#[derive(Debug)]
pub struct IndicatifProgressHandler {
    main_bar: ProgressBar,
    multi_pb: MultiProgress,
    imageboard_theme: ImageBoards, // To select styles for download bars
}

impl IndicatifProgressHandler {
    /// Initialize the main progress bar and the stat counters.
    ///
    /// The style that the main progress bar will use is based on the predefined styles for each variant of the ['ImageBoards' enum](ibdl_common::ImageBoards)
    pub fn new(initial_len: u64, imageboard: ImageBoards) -> Self {
        let template = BarTemplates::new(imageboard);
        let main_style = master_progress_style(&template);
        let bar = ProgressBar::new(initial_len).with_style(main_style);
        bar.set_draw_target(ProgressDrawTarget::stderr());
        bar.enable_steady_tick(Duration::from_millis(100));

        // Initialize the bars
        let multi = MultiProgress::new();
        let main = multi.add(bar);

        // The original ProgressCounter had AtomicUsize and AtomicU64 counters.
        // If the CLI needs to query these counts independently of the progress bar display,
        // they can be added here and updated by the trait methods.
        // For now, we rely on the ProgressBar's internal state or the counts returned by core functions.

        Self {
            main_bar: main,
            multi_pb: multi,
            imageboard_theme: imageboard,
        }
    }
}

#[derive(Debug)]
struct IndicatifDownloadProgressUpdater {
    bar: ProgressBar,
}

impl DownloadProgressUpdater for IndicatifDownloadProgressUpdater {
    fn set_progress(&self, bytes_downloaded: u64) {
        self.bar.set_position(bytes_downloaded);
    }

    fn set_total_size(&self, total_size: u64) {
        if self.bar.length().is_none() || self.bar.length() != Some(total_size) {
            self.bar.set_length(total_size);
        }
    }

    fn finish(&self) {
        self.bar.finish_and_clear(); // Or just .finish() if you want it to remain
    }
}

impl ProgressListener for IndicatifProgressHandler {
    fn set_main_total(&self, total: u64) {
        self.main_bar.set_length(total);
    }

    fn inc_main_total(&self, delta: u64) {
        self.main_bar.inc_length(delta);
    }

    fn main_tick(&self) {
        self.main_bar.inc(1);
    }

    fn main_inc_by(&self, delta: u64) {
        self.main_bar.inc(delta);
    }

    fn main_done(&self) {
        self.main_bar.finish_with_message("All posts processed.");
    }

    fn add_download_task(
        &self,
        name: String,
        total_size: Option<u64>,
    ) -> Box<dyn DownloadProgressUpdater> {
        let template = BarTemplates::new(self.imageboard_theme);
        let style = download_progress_style(&template);

        let pb = ProgressBar::new(total_size.unwrap_or(0)) // Or handle unknown size differently
            .with_style(style)
            .with_message(name); // Display the name (e.g., filename)
        pb.set_draw_target(ProgressDrawTarget::stderr());

        let managed_pb = self.multi_pb.add(pb);

        Box::new(IndicatifDownloadProgressUpdater { bar: managed_pb })
    }

    fn log_event(&self, log_type: LogType, target: &str, message: &str) {
        let formatted_message = match log_type {
            LogType::Info => format!("{} {}", target.bold(), message),
            LogType::Skip => {
                format!(
                    "{} {} {}",
                    target.blue().italic(),
                    message.green().bold(),
                    "Skipping...".green().bold()
                )
            }
            LogType::Rename => {
                format!("{} {}", target.blue().italic(), message.green().bold(),)
            }
            LogType::Remove => {
                format!(
                    "{} {} {}",
                    target.blue().italic(),
                    message.red().bold(),
                    "Removed.".red().bold()
                )
            }
            LogType::Success => {
                format!("{} {}", target.blue().italic(), message.green().bold(),)
            }
            LogType::Warning => format!(
                "{} {} {}",
                target.blue().italic(),
                message.yellow().bold(),
                "Warning.".yellow().bold()
            ),
            LogType::Error => format!(
                "{} {} {}",
                target.blue().italic(),
                message.red().bold(),
                "Error.".red().bold()
            ),
        };

        self.main_bar.println(formatted_message);
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

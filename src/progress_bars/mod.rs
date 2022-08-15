use indicatif::{HumanBytes, ProgressState, ProgressStyle};
use std::fmt::Write;

const PROGRESS_CHARS: &str = "━━";

pub struct BarTemplates {
    pub main: &'static str,
    pub download: &'static str,
}

impl Default for BarTemplates {
    fn default() -> Self {
        Self {
            main: "{spinner:.green.bold} {elapsed_precise:.bold} {wide_bar:.green/white.dim} {percent:.bold}  {pos:.green} ({files_sec:.bold} | eta. {eta})",
            download: "{spinner:.green.bold} {bar:40.green/white.dim} {percent:.bold} | {byte_progress:.green} @ {bytes_per_sec:>13.red} (eta. {eta:.blue})",
        }
    }
}

pub fn master_progress_style(templates: BarTemplates) -> ProgressStyle {
    ProgressStyle::default_bar()
        .template(templates.main)
        .unwrap()
        .with_key("pos", |state: &ProgressState, w: &mut dyn Write| {
            write!(w, "{}/{}", state.pos(), state.len().unwrap()).unwrap()
        })
        .with_key("percent", |state: &ProgressState, w: &mut dyn Write| {
            write!(w, "{:>3.0}%", state.fraction() * 100_f32).unwrap()
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

pub fn download_progress_style(templates: BarTemplates) -> ProgressStyle {
    ProgressStyle::default_bar()
        .template(templates.download)
        .unwrap()
        .with_key("percent", |state: &ProgressState, w: &mut dyn Write| {
            write!(w, "{:>3.0}%", state.fraction() * 100_f32).unwrap()
        })
        .with_key("byte_progress", |state: &ProgressState, w: &mut dyn Write| {
            write!(w, "{}/{}", HumanBytes(state.pos()), HumanBytes(state.len().unwrap())).unwrap()
        })
        .progress_chars(PROGRESS_CHARS)
}

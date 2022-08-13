use clap::ValueEnum;
use log::debug;

mod common;
pub mod danbooru;
pub mod e621;
pub mod konachan;
mod macros;
pub mod realbooru;
pub mod rule34;

#[derive(Debug, Copy, Clone, ValueEnum)]
pub enum ImageBoards {
    Danbooru,
    E621,
    Rule34,
    Realbooru,
    Konachan,
}

impl ToString for ImageBoards {
    fn to_string(&self) -> String {
        match self {
            ImageBoards::Danbooru => String::from("danbooru"),
            ImageBoards::E621 => String::from("e621"),
            ImageBoards::Rule34 => String::from("rule34"),
            ImageBoards::Realbooru => String::from("realbooru"),
            ImageBoards::Konachan => String::from("konachan"),
        }
    }
}

impl ImageBoards {
    pub fn user_agent(&self) -> String {
        let app_name = "Rust Imageboard Downloader";
        let variant = match self {
            ImageBoards::Danbooru => " (by danbooru user FerrahWolfeh)",
            ImageBoards::E621 => " (by e621 user FerrahWolfeh)",
            _ => "",
        };
        let ua = format!("{}/{}{}", app_name, env!("CARGO_PKG_VERSION"), variant);
        debug!("Using user-agent: {}", ua);
        ua
    }
}

// impl fmt::Display for ImageBoards {
//     fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
//         fmt.pad(self.as_str())
//     }
// }
//
// impl ImageBoards {
//     pub fn as_str(&self) -> &'static str {
//         IMAGEBOARD_NAMES[*self as usize]
//     }
//
//     fn from_usize(u: usize) -> Option<ImageBoards> {
//         match u {
//             1 => Some(ImageBoards::Danbooru),
//             2 => Some(ImageBoards::E621),
//             3 => Some(ImageBoards::Rule34),
//             4 => Some(ImageBoards::RealBooru),
//             _ => None,
//         }
//     }
// }
//
// impl FromStr for ImageBoards {
//     type Err = ParseError;
//
//     fn from_str(imgboard: &str) -> Result<ImageBoards, Self::Err> {
//         IMAGEBOARD_NAMES
//             .iter()
//             .position(|&name| str::eq_ignore_ascii_case(name, imgboard))
//             .map(|p| ImageBoards::from_usize(p).unwrap())
//             .ok_or(ParseError(()))
//     }
// }
//
// #[allow(missing_copy_implementations)]
// #[derive(Debug, PartialEq)]
// pub struct ParseError(());
//
// impl fmt::Display for ParseError {
//     fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
//         fmt.write_str(IMAGEBOARD_PARSE_ERROR)
//     }
// }
//
// impl error::Error for ParseError {}

use clap::ValueEnum;

pub mod danbooru;
pub mod e621;
pub mod realbooru;
pub mod rule34;

pub const DANBOORU_UA: &str = "Rust Imageboard Downloader/0.5.0 (by danbooru user FerrahWolfeh)";
pub const E621_UA: &str = "Rust Imageboard Downloader/0.5.0 (by e621 user FerrahWolfeh)";
pub const GENERIC_UA: &str = "Rust Imageboard Downloader/0.5.0";

#[repr(usize)]
#[derive(Debug, Copy, Clone, ValueEnum)]
pub enum ImageBoards {
    Danbooru,
    E621,
    Rule34,
    Realbooru,
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

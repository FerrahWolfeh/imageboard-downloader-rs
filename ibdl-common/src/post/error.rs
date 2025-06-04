use std::{io, num::TryFromIntError};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum PostError {
    #[error("This file was already correctly downloaded")]
    CorrectFileExists,

    #[error("Failed to access file: {source}")]
    FileIOError {
        #[from]
        source: io::Error,
    },

    #[error("Failed to print line to Progress Bar: {message}")]
    ProgressBarPrintFail { message: String },

    #[error("Failed to connect to download URL: {source}")]
    ConnectionFail {
        #[from]
        source: reqwest::Error,
    },

    #[error("Post URL is valid but original file doesn't exist")]
    RemoteFileNotFound,

    #[error("Error while fetching chunk: {message}")]
    ChunkDownloadFail { message: String },

    #[error("Failed to start thread for writing file to destination cbz: {msg}")]
    ZipThreadStartError { msg: String },

    #[error("Failed to write file to destination cbz: {message}")]
    ZipFileWriteError { message: String },

    #[error("Int conversion failed (maybe size is too large?)")]
    IntConversion(#[from] TryFromIntError),

    #[error("Post has an unknown extension: {message}")]
    UnknownExtension { message: String },
}

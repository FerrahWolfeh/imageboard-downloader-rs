#![deny(clippy::nursery, clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::struct_field_names)]

//! All internal logic for interacting with and downloading from imageboard websites.

extern crate ibdl_common;

pub mod auth;
pub mod blacklist;
pub mod error;
mod extractor;
pub mod extractor_config;
pub mod imageboards;
pub mod prelude;
mod test;

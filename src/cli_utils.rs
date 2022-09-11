use anyhow::Error;

use imageboard_downloader::imageboards::{post::PostQueue, queue::summary::SummaryFile};

use std::{fs::File, io::Read};
use zip::ZipArchive;

pub fn filter_by_summary_zip(
    queue: &mut PostQueue,
    zip: &mut ZipArchive<&File>,
) -> Result<(), Error> {
    if let Ok(mut raw) = zip.by_name("00_summary.json") {
        let mut summary_slice = vec![];
        raw.read_to_end(&mut summary_slice)?;

        let json_data = serde_json::from_slice::<SummaryFile>(&summary_slice)?;
        queue.posts.retain(|c| c.id > json_data.last_downloaded.id);
    }

    Ok(())
}

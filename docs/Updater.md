# Update already downloaded gallery

⚠ This is an experimental feature and it's subject to change a lot over time.

## About

⚠ **This is currently incompatible with [CBZ mode](CBZ.md).**

When downloading a list of tags, *imageboard_downloader* will also create a `.00_download_summary.bin` file in the same directory right after it finishes. This is a ZSTD-compressed [bincode](https://github.com/bincode-org/bincode) file containing info about the **post with the highest id** found in the latest download run.

**❗ Closing or cancelling the download while it's still running will prevent the creation of the summary file.**

When run with the `--update` flag while the output dir specified in the `-o` option is the same as the previous run, *imageboard_downloader*, after scanning the posts as usual, will attempt to read the summary file in case it exists and will remove all other posts from the queue that have an id **lower** than the one found in the summary, and then will download only the remaining posts in the list as usual. In case there are no new posts, the program will gracefully exit.

- Deleting files in the gallery and later running with the update flag enabled, will not redownload the files like the utility usually does.

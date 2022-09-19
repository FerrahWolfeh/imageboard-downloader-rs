# Imageboard Downloader

*imageboard-downloader-rs* is a command-line multi image gallery downloader made in **Rust** with a very simple, yet
extensible API.

It is a cross-platform tool with speed, simple cli interface and multiple simultaneous downloads as its main focus.

*imageboard_downloader_rs* has a hardcoded limit of **100 pages** per download session to prevent API rate-limiting and put less strain on the imageboard's servers.

⚠ **Avoid downloading single tag selections that span ~100k posts alone without using the download limiter. Be reasonate!**

![Running example](assets/mini-ex.gif)

## Features

- [x] Multiple simultaneous downloads.
- [x] Authentication and user blacklist.
- [x] Download limit.
- [x] Global blacklist. [See more](docs/Global_Blacklist.md)
- [x] Store downloads in `cbz` file. [See more](docs/CBZ.md)
- [x] Update already downloaded galleries. [See more](docs/Updater.md)

## Installation

Currently, you can install the latest version using `cargo` or download from **Releases**

```bash
cargo install imageboard_downloader
```

Or by cloning this repository and building it yourself

```bash
git clone https://gitlab.com/FerrahWolfeh/imageboard-downloader-rs.git

cd imageboard-downloader-rs

cargo build --release

cargo run --release -- "your_tag" "your_another_tag_(cool)" -o ~/
```

The final binary will be located at `target/release/imageboard_downloader`

***Windows releases coming someday...***

## Usage

To use the utility simply call it with space-separated tags

```bash
imageboard_downloader [OPTIONS] <TAGS>...
```

Or run with `cargo`

```bash
cargo run -- [OPTIONS] <TAGS>...
```

See more details with `imageboard_downloader --help`.

***

## Examples

### Download images from danbooru with specified tags

```bash
imageboard_downloader "skyfire_(arknights)"
```

In case you want to authenticate with danbooru, use the `--auth` flag only once. Then all subsequent downloads will use authentication as well.

***

### Download images starting from page 10

```bash
imageboard_downloader "skyfire_(arknights)" -s 10
```

***

### Download only images with "safe" rating from e621

```bash
imageboard_downloader -i e621 "ash_(pokemon)" "pikachu" --safe-mode
```

***

### Download images from rule34 with 20 simultaneous downloads

```bash
imageboard_downloader -i rule34 -d 20 "moe"
```

***

### Save downloaded images with their id instead of md5 as filename

```bash
imageboard_downloader -i e621 "wolf" "anthro" --id
```

***

By default, the program will download files to your current dir with the following structure `./<gallery_name>/tag1 tag2 .../<file_md5>.png`. In case you want to download files to another place use:

```bash
imageboard_downloader "kroos_(arknights)" -o /any/other/dir
```

This will save files in `/any/other/dir/danbooru/kroos_(arknights)/<file_md5>.png`
If the specified directory does not exist, it will be created.

***

### Update already downloaded gallery

When using the `--update` flag and using a previous tag and dir selection, the utility will only download images newer than the last post downloaded in a previous successful run.

```bash
imageboard_downloader "kroos_(arknights)" -o /any/other/dir --update
```

***

## Inspiration and References

- gallery-dl                         <https://github.com/mikf/gallery-dl>
- trauma (download workflow)         <https://github.com/rgreinho/trauma>
- Av1an (mainly the progress bars)   <https://github.com/master-of-zen/av1an>
- e621_downloader (part of e621 api) <https://github.com/McSib/e621_downloader>

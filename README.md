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
- [x] Custom websites support.
- [x] Global blacklist. [See more](docs/Global_Blacklist.md)
- [x] Store downloads in `cbz` file. [See more](docs/CBZ.md)

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

cargo run --release -- search "your_tag" "your_another_tag_(cool)" -o ~/
```

The final binary will be located at `target/release/imageboard_downloader`

***Windows releases coming someday...***

## Usage

### The utility has 3 main operating modes:

#### 1. Tag Search
This mode is the former default mode of the utility, where it will fetch all posts with a tag-based search
```bash
cargo run --release -- search [OPTIONS] <TAGS>...
```

#### 2. Post download
This mode is meant for downloading a single or a select few posts byt inputting their id
```bash
cargo run --release --  post [OPTIONS] <POST_IDS>...
```

#### 3. Pool download
This mode is for downloading entire groups of organized posts (pools)
```bash
cargo run --release -- pool [OPTIONS] <POOL_ID>
```

Each mode has their own unique set of options, see more details with `imageboard_downloader --help` or `cargo run --release -- --help`.

***

## Examples

### Download images from danbooru with specified tags

```bash
imageboard_downloader search "skyfire_(arknights)"
```

In case you want to authenticate with danbooru or e621, use the `--auth` flag only once. Then all subsequent downloads will use authentication as well.

***

### Download images starting from page 10

```bash
imageboard_downloader search "skyfire_(arknights)" -s 10
```

***

### Download only images with "safe" rating from e621

```bash
imageboard_downloader search -i e621 "ash_(pokemon)" "pikachu" --safe-mode
```

***

### Download images from rule34 with 20 simultaneous downloads

```bash
imageboard_downloader search -i rule34 -d 20 "moe"
```

***

### Save downloaded images with their id instead of md5 as filename

```bash
imageboard_downloader search -i e621 "wolf" "anthro" --id
```

***

By default, the program will download files to your current dir. In case you want to download files to another place use:

```bash
imageboard_downloader "kroos_(arknights)" -o /any/other/dir
```

This will save files in `/any/other/dir/<file>.png`
If the specified directory does not exist, it will be created.

### Download posts with annotated tags
In order to download posts and save their tags along with them in a `.txt` file, just run the app like this:
```bash
cargo run --release -- post -o /whenever --annotate 123 456 69420
```

***

## Inspiration and References

- gallery-dl                         <https://github.com/mikf/gallery-dl>
- trauma (download workflow)         <https://github.com/rgreinho/trauma>
- Av1an (mainly the progress bars)   <https://github.com/master-of-zen/av1an>
- e621_downloader (part of e621 api) <https://github.com/McSib/e621_downloader>

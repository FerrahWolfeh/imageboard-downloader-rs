# Imageboard Downloader

*imageboard-downloader-rs* a command-line multi image gallery downloader made in rust.
It is a cross-platform tool with speed, simple cli interface and multiple simultaneous downloads as it's main features.

Dependencies
============

- OpenSSL >= 1.1.1


Installation
============

While there aren't any stable releases yet, it is recommended to clone the git repository

```bash
    git clone https://gitlab.com/FerrahWolfeh/imageboard-downloader-rs.git
```

And build/run the code with `cargo`

```bash
    cd imageboard-downloader-rs
    cargo build --release
    cargo run --release -- "your_tag" "your_another_tag_(cool)" -o ~/
```

The final binary will be located at `target/release/imageboard_downloader`

Usage
=====

To use the utility simply call it with space-separated tags

```bash

    imageboard_downloader [OPTIONS] <TAGS>...

```
Or run with `cargo`

```bash
    cargo run -- [OPTIONS] <TAGS>...
```

See more details with `imageboard_downloader --help`.


Examples
--------

Download images from danbooru with specified tags:

```bash

    imageboard_downloader "skyfire_(arknights)"

```
In case you want to authenticate with danbooru, use the `--auth` flag only once. Then all subsequent downloads will use authentication as well.

Download only images with "safe" rating from e621 (also works with danbooru/konachan):

```bash
    imageboard_downloader -i e621 "ash_(pokemon)" "pikachu" --safe-mode
```

Download images from rule34 with 100 simultaneous downloads:
```bash

    imageboard_downloader -i rule34 -d 100 "moe"

```

| By default, the program will download files to your current dir with the following structure `./<gallery_name>/tag1+tag2+.../<file_md5>.png`. In case you want to download files to another place use:
```bash

    imageboard_downloader "kroos_(arknights)" -o /any/other/dir

```
This will save files in `/any/other/dir/danbooru/kroos_(arknights)/<file_md5>.png`
If the specified directory does not exist, it will be created.

Save downloaded images with their id instead of md5 as filename:
```bash

    imageboard_downloader -i e621 "wolf" "anthro" --id

```



Inspiration and References
==========================

* gallery-dl                         https://github.com/mikf/gallery-dl
* trauma (download workflow)         https://github.com/rgreinho/trauma
* Av1an (mainly the progress bars)   https://github.com/master-of-zen/av1an
* e621_downloader (part of e621 api) https://github.com/McSib/e621_downloader



# Global Blacklist

## About

When running for the first time, the utility will create `$XDG_CONFIG_HOME/imageboard_downloader/blacklist.toml` with the following contents:

```toml
[blacklist]
global = [] # Place in this array all the tags that will be excluded from all imageboards

# Place in the following all the tags that will be excluded from specific imageboards 

danbooru = []

e621 = []

realbooru = []

rule34 = []

gelbooru = []

konachan = []
```

This file serves as a global blacklist for all imageboards even when the user is not logged into the imageboard via the `--auth` flag.

Placing strings inside `global` such as

```toml
global = ["a_nasty_tag", "other_nasty_tag"]
```

will make the Queue Manager drop all posts that contain one or all of the tags when downloading from **any** imageboard.

While placing strings inside any other array, will make the Queue Manager drop all posts with those tags while downloading from that imageboard.

## Disabling

To disable the Blacklist Filtering, which includes user-defined blacklisted tags and the Global Blacklist, just pass the `--disable-blacklist` flag while running *imageboard_downloader*:

```bash
imageboard_downloader   \ 
    -i e621             \ # Get images from E621
    -d 10               \ # Download 10 items at the same time
    --disable-blacklist \ # Ignore all blacklists
    "fox" "multi_tail"    # Tags to search
```

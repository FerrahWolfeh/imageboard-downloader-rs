# Store downloaded posts in `cbz` archive

## About

When ran with the `--cbz` flag, instead of downloading all images to `./<imageboard>/<tag1 tag2>/<image>.png`, the utility will save all of them in real time to a `cbz` file in `./<imageboard>/<tag1 tag2>.cbz` using `store` compression (so, no compression).

### File structure

After download, the images will be saved inside the zip file as follows:

```bash
├── 00_summary.json
├── Explicit
│   ├── image.jpeg
│   └── image.jpeg
├── Questionable
│   └── image.png
├── Safe
│   ├── image.gif
│   └── image.jpeg
└── Unknown
    ├── image.jpeg
    └── image.jpeg
```

Each file will be located in it a dir that matches it's `rating` tag.

At the top level, there will be a `00_summary.json` file which will have some general info about the downloaded posts present inside the `cbz`.

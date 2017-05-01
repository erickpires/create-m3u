## About
A very simple program to sweep directories and write and M3U playlist file with all audio
files found under the given directory. The standard behaviour is to do a recursive search
in all the directories under the directory passed as input.

This program uses a Rust [wrapper](https://github.com/erickpires/rust-mediainfo) around
the [MediaInfo Library](https://mediaarea.net/en/MediaInfo) to retrieve some file metadata.

## Usage

```
create_m3u [DIR LIST]
```

`DIR LIST` can be any number of directories. A playlist is written for each directory
in the list. The program tries to create a file using the same name as the directory and
falls back to `playlist.m3u` if something goes wrong.

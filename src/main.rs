#[macro_use]
extern crate lazy_static;
extern crate mediainfo;

use mediainfo::MediaInfo;

use std::fmt::Display;
use std::fmt::Formatter;

use std::path::Path;
use std::path::PathBuf;

use std::cmp::Ordering;

use std::collections::HashSet;

use std::fs::File;
use std::fs::read_dir;
use std::fs::canonicalize;
use std::io::Write;

use std::io::stderr;
use std::env;

#[derive(Debug)]
struct M3uFileInfo<'a> {
    path             : &'a Path,
    title            : Option<String>,
    artist           : Option<String>,
    duration_in_secs : Option<u32>,

    // NOTE(erick) : Not actually used to output the file
    // but useful for sorting.
    track_number     : Option<u32>,
    album            : Option<String>
}

impl<'a> M3uFileInfo<'a> {
    fn new(path: &'a Path) -> M3uFileInfo<'a> {
        M3uFileInfo {
            path             : path,
            title            : None,
            artist           : None,
            duration_in_secs : None,
            track_number     : None,
            album            : None
        }
    }

    fn add_title(&mut self, title: String) {
        self.title = Some(title);
    }
    fn add_artist(&mut self, artist: String) {
        self.artist = Some(artist);
    }
    fn add_duration(&mut self, duration: u32) {
        self.duration_in_secs = Some(duration);
    }
    fn add_track_number(&mut self, number: u32) {
        self.track_number = Some(number);
    }
    fn add_album(&mut self, album: String) {
        self.album = Some(album);
    }
}

impl<'a> PartialEq for M3uFileInfo<'a> {
    fn eq(&self, rhs: &M3uFileInfo) -> bool {
        if self.track_number != rhs.track_number { return false; }
        if self.artist != rhs.artist { return false; }
        if self.album != rhs.album { return false; }
        if self.title != rhs.title { return false; }
        if self.duration_in_secs != rhs.duration_in_secs { return false; }
        if self.path != rhs.path { return false; }

        true
    }
}

impl<'a> Eq for M3uFileInfo<'a> {}

impl<'a> PartialOrd for M3uFileInfo<'a> {
    fn partial_cmp(&self, rhs: &M3uFileInfo<'a>) -> Option<Ordering> {
        Some(self.cmp(rhs))
    }
}

impl<'a> Ord for M3uFileInfo<'a> {
    fn cmp(&self, rhs: &M3uFileInfo<'a>) -> Ordering {
        if self == rhs { return Ordering::Equal; }

        let artist_cmp = self.artist.cmp(&rhs.artist);
        if artist_cmp != Ordering::Equal { return artist_cmp; }

        let album_cmp = self.album.cmp(&rhs.album);
        if album_cmp != Ordering::Equal { return album_cmp; }

        let number_cmp = self.track_number.cmp(&rhs.track_number);
        if number_cmp != Ordering::Equal { return number_cmp; }

        let title_cmp = self.title.cmp(&rhs.title);
        if title_cmp != Ordering::Equal { return title_cmp; }

        let duration_cmp = self.duration_in_secs.cmp(&rhs.duration_in_secs);
        if duration_cmp != Ordering::Equal { return duration_cmp; }

        let path_cmp = self.path.cmp(&rhs.path);
        if path_cmp != Ordering::Equal { return path_cmp; }

        Ordering::Equal
    }
}

impl<'a> Display for M3uFileInfo<'a> {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self.path)?;

        if self.title.is_some() {
            write!(f, "\n\tTitle: {}", self.title.as_ref().unwrap())?;
        }
        if self.track_number.is_some() {
            write!(f, " : #{}", self.track_number.as_ref().unwrap())?;
        }
        if self.artist.is_some() {
            write!(f, "\n\tArtist: {}", self.artist.as_ref().unwrap())?;
        }
        if self.album.is_some() {
            write!(f, "\n\tAlbum: {}", self.album.as_ref().unwrap())?;
        }
        if self.duration_in_secs.is_some() {
            write!(f, "\n\tDuration: {}s", self.duration_in_secs.as_ref().unwrap())?;
        }

        Ok( () )
    }
}

lazy_static! {
    static ref VALID_AUDIO_FORMATS: HashSet<&'static str> = {
        let mut result = HashSet::new();
        result.insert("mp3");
        result.insert("ogg");
        result.insert("flac");
        result.insert("wav");
        result.insert("m4a");
        result.insert("wma");
        result
    };
}

fn main() {
    const RECURSE: bool = true;

    let args = env::args();
    if args.len() > 1 {
        let args = args.skip(1);
        for arg in args {
            sweep_directory(arg);
        }
    } else {
        sweep_directory(".".to_string());
    }

    #[allow(unused_must_use)]
    fn sweep_directory(directory_path: String) {
        let path_to_search = canonicalize(directory_path);
        if path_to_search.is_err() {
            writeln!(stderr(), "Invalid path:\n\t{:?}", path_to_search.unwrap_err());
            return;
        }

        let path_to_search = path_to_search.unwrap();
        if !path_to_search.is_dir() {
            writeln!(stderr(), "Path is not a directory:\n\t{:?}", path_to_search);
            return;
        }

        let mut audio_files: Vec<PathBuf> = Vec::new();
        append_audio_files(&mut audio_files, &path_to_search, RECURSE);

        if audio_files.len() == 0 {
            writeln!(stderr(),
                     "No audio files found at: {:?}", &path_to_search);
            return;
        }

        let mut files_info = get_audio_files_info(&audio_files, &path_to_search);

        // NOTE(erick): Let's sort the vector to output in a nice order.
        files_info.sort();

        let write_result = write_m3u_file(&files_info, &path_to_search);
        if write_result.is_err() {
            writeln!(stderr(),
                     "Failed to write playlist for directory:\n\t{:?}\nError: {:?}",
                     path_to_search, write_result.unwrap_err());
            return;
        }
    }
}

// NOTE(erick): This function appends data to a Vec instead of returning one
// because this way the recursion is easy to implement without unnecessary
// memory and time costs.
#[allow(unused_must_use)]
fn append_audio_files(audio_files: &mut Vec<PathBuf>,
                      path_to_search: &PathBuf, recurse: bool) {
    let dir_iterator = read_dir(path_to_search);
    if dir_iterator.is_err() {
        writeln!(stderr(), "Failed to read directory:\n\t{:?}", path_to_search);
        return;
    }

    let dir_iterator = dir_iterator.unwrap();
    for file in dir_iterator {
        let file = file;
        if file.is_err() {
            writeln!(stderr(), "Could not reach file:\n\t{:?}", file.unwrap_err());
            return;
        }

        let file = file.unwrap();
        let file_path = file.path();
        let metadata = file.metadata();
        if metadata.is_err() {
            writeln!(stderr(), "Failed to get metadata for file:\n\t{:?}", file);
            return;
        }

        let metadata = metadata.unwrap();
        if metadata.is_dir() && recurse {
            append_audio_files(audio_files, &file_path, recurse);
        } else if metadata.is_file() && keep_file(&file_path){
            audio_files.push(file_path);
        }
    }
}

fn keep_file(file_path: &PathBuf) -> bool {

    let extension = file_path.extension();
    if extension.is_none() { return false; }

    let extension = extension.unwrap().to_str();
    if extension.is_none() { return false; }

    let extension = extension.unwrap();
    if !VALID_AUDIO_FORMATS.contains(extension) { return false; }

    true
}

fn get_audio_files_info<'a>(audio_files: &'a Vec<PathBuf>,
                        path_to_search: &'a PathBuf) -> Vec<M3uFileInfo<'a > > {
    let mut mediainfo = MediaInfo::new();
    let mut files_info = Vec::new();
    for audio_file in audio_files {
        let file_path;

        // NOTE(erick): Try to use a relative path, if it does not succeed
        // fallback to the absolute path.
        let relative_path = audio_file.strip_prefix(path_to_search);
        if relative_path.is_ok() {
            let relative_path = relative_path.unwrap();
            file_path = relative_path;
        } else {
            file_path = audio_file.as_path();
        }

        let mut file_info = M3uFileInfo::new(file_path);

        let path_as_str = audio_file.to_str();
        if path_as_str.is_none() { continue; }

        let path_as_str = path_as_str.unwrap();
        mediainfo.open(path_as_str);

        let artist = mediainfo.get_performer();
        let track_name = mediainfo.get_title();
        let duration = mediainfo.get_duration_ms();
        let track_number = mediainfo.get_track_number();
        let album = mediainfo.get_album();

        if artist.len() != 0 { file_info.add_artist(artist); }
        if album.len() != 0 { file_info.add_album(album); }
        if track_name.len() != 0 { file_info.add_title(track_name); }
        if let Some(ms) = duration { file_info.add_duration(ms / 1000); }
        if let Some(num) = track_number { file_info.add_track_number(num); }

        mediainfo.close();
        files_info.push(file_info);
    }

    files_info
}

fn write_m3u_file(files_info: &Vec<M3uFileInfo>, path_to_search: &PathBuf)
                  -> Result<(), std::io::Error> {
    // NOTE(erick): Tries to create a name for the m3u file based on
    // the directory been searched. Falls back to 'playlist.m3u'
    // otherwise
    let mut playlist_filename = String::from("playlist.m3u");
    let directory_name = path_to_search.file_stem();
    if directory_name.is_some() {
        let directory_name_str = directory_name.unwrap().to_str();
        if directory_name_str.is_some() {
            playlist_filename = String::from(directory_name_str.unwrap());
            playlist_filename.push_str(".m3u");
        }
    }

    let m3u_file_path = path_to_search.join(playlist_filename.as_str());
    let mut m3u_file = File::create(m3u_file_path)?;

    // Write the M3U file header
    m3u_file.write_all(b"#EXTM3U\n")?;

    for file_info in files_info {
        let path_str = file_info.path.to_str();
        // NOTE(erick): If we can't get the path there's nothing
        // we can do about it. Just continue to the next file.
        if path_str.is_none() { continue; }
        let duration = file_info.duration_in_secs;
        let artist = &file_info.artist;
        let track_title = &file_info.title;

        // Write the information only if it is available
        if duration.is_some() &&
            artist.is_some() &&
            track_title.is_some() {
                m3u_file.write_fmt(format_args!("#EXTINF:{},{} - {}\n",
                                                duration.unwrap(),
                                                artist.as_ref().unwrap(),
                                                track_title.as_ref().unwrap()))?;
            }

        // Write the path followed by a newline.
        let path_str = path_str.unwrap();
        m3u_file.write_all(path_str.as_bytes())?;
        m3u_file.write_all(b"\n")?;
    }

    Ok( () )
}

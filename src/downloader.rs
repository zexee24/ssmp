use std::{fs, path::PathBuf, process::Command};

use id3::{Tag, TagLike};
use rustube::{blocking::Video, Id};

use crate::{conf::Configuration, format::Format, song::Song};

pub(crate) fn download(url: String) -> Result<Song, String> {
    let id = Id::from_raw(url.as_str());
    return match id {
        Ok(id) => {
            let video = Video::from_id(id.as_owned()).unwrap();
            let best_video = video
                .streams()
                .iter()
                .filter(|stream| stream.includes_audio_track)
                .min_by_key(|stream| stream.quality_label)
                .ok_or("Error in getting stream")?;
            let owned = Configuration::get_conf().owned_path;
            let path = best_video
                .blocking_download_to_dir(owned)
                .map_err(|e| format!("Failed the download: {e}"))?;
            let name = video.title();
            let file_path = change_format_and_name_better(name, path).unwrap();
            let song = Song {
                name: name.to_string(),
                artist: Some(video.video_details().author.clone()),
                url: Some(url),
                path: file_path,
                format: Format::MP3,
            };
            if let Err(e) = set_metadata(song.clone()) {
                println!("Error when writing metadata: {:?}", e)
            }
            Ok(song)
        }
        Err(e) => Err(format!("Unable to get video id: {e}")),
    };
}

pub(crate) fn change_format_and_name_better(name: &str, path: PathBuf) -> Result<PathBuf, String> {
    let new_file_name = generate_filename(name, Format::MP3);
    let mut new_loc = Configuration::get_conf().owned_path;
    new_loc.push(new_file_name);
    println!("New loc = {:?}", new_loc);

    Command::new("ffmpeg")
        .args(["-i", path.to_str().unwrap(), new_loc.to_str().unwrap()])
        .output()
        .expect("Failed the command");
    fs::remove_file(path).unwrap();
    Ok(new_loc)
}

fn generate_filename(name: &str, format: Format) -> String {
    name.replace(['/', '\\'], "-")
        .replace([':', '.', '!', '?'], "")
        .make_ascii_lowercase();
    name.to_owned() + &Format::filetype_to_extension(format).unwrap_or(".mp3".to_owned())
}

fn set_metadata(song: Song) -> Result<(), id3::Error> {
    let mut tag = Tag::read_from_path(song.path.clone()).unwrap_or(Tag::new());
    tag.set_title(song.name);
    if let Some(artist) = song.artist {
        tag.set_artist(artist)
    } else {
        tag.set_artist("")
    }
    tag.set_album("");
    if let Some(url) = song.url {
        tag.set_text("url", url)
    }
    tag.write_to_path(song.path, id3::Version::Id3v22)
}

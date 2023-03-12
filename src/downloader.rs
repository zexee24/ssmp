use std::{fs, io::Cursor, path::PathBuf, process::Command};

use id3::{frame::Picture, Frame, Tag, TagLike};
use image::{DynamicImage, EncodableLayout, ImageOutputFormat};
use rustube::{blocking::Video, Id};

use crate::{conf::Configuration, format::Format, song::Song};

pub(crate) fn download(url: String) -> Result<Song, String> {
    let id = Id::from_raw(url.as_str());
    return match id {
        Ok(id) => {
            let video = Video::from_id(id.as_owned()).unwrap();
            let img = if let Some(thumbnal_max_res) = &video
                .video_details()
                .thumbnails
                .iter()
                .max_by_key(|t| t.width)
            {
                get_image(thumbnal_max_res.url.clone())
            } else {
                None
            };

            let path = download_best_stream(&video).ok_or("Error in downloading stream")?;
            let name = video.title();
            let file_path = change_format_and_name_better(name, path).unwrap();
            let song = Song {
                name: name.to_string(),
                artist: Some(video.video_details().author.clone()),
                url: Some(url),
                path: file_path,
                format: Format::MP3,
            };
            if let Err(e) = set_metadata(song.clone(), img) {
                println!("Error when writing metadata: {:?}", e)
            }
            Ok(song)
        }
        Err(e) => Err(format!("Unable to get video id: {e}")),
    };
}

fn download_best_stream(video: &Video) -> Option<PathBuf> {
    let owned = Configuration::get_conf().owned_path;
    let mut streams = video.streams().clone();
    streams.sort_by_key(|s| s.audio_sample_rate);
    for stream in streams {
        if stream.includes_video_track {
            continue;
        }
        match stream.blocking_download_to_dir(&owned) {
            Ok(stream) => return Some(stream),
            Err(e) => println!("Stream {:?} failed, trying next one", e),
        }
    }
    None
}

pub(crate) fn change_format_and_name_better(name: &str, path: PathBuf) -> Result<PathBuf, String> {
    let new_file_name = generate_filename(name, Format::MP3);
    let mut new_loc = Configuration::get_conf().owned_path;
    new_loc.push(new_file_name);

    Command::new("ffmpeg")
        .args(["-i", path.to_str().unwrap(), new_loc.to_str().unwrap()])
        .output()
        .expect("Failed the command");
    fs::remove_file(path).unwrap();
    Ok(new_loc)
}

fn generate_filename(name: &str, format: Format) -> String {
    let n = name
        .replace(['/', '\\'], "-")
        .replace([':', '.', '!', '?'], "")
        .to_ascii_lowercase();
    n + &Format::filetype_to_extension(format).unwrap_or(".mp3".to_owned())
}

fn get_image(url: String) -> Option<DynamicImage> {
    let resp = reqwest::blocking::get(url).ok()?;
    let bytes = resp.bytes().unwrap();
    image::io::Reader::new(Cursor::new(bytes.as_bytes()))
        .with_guessed_format()
        .ok()?
        .decode()
        .ok()
}

#[test]
fn test_filename() {
    let new_name = generate_filename("Heilutaan / Eurobeat Remix", Format::MP3);
    assert_eq!(new_name, "heilutaan - eurobeat remix.mp3")
}

fn set_metadata(song: Song, img: Option<DynamicImage>) -> Result<(), id3::Error> {
    let mut tag = Tag::read_from_path(song.path.clone()).unwrap_or(Tag::new());
    tag.set_title(song.name.replace(['/', '\\'], "-"));
    if let Some(artist) = song.artist {
        tag.set_artist(artist)
    } else {
        tag.set_artist("")
    }
    tag.set_album("");
    if let Some(url) = song.url {
        let frame = Frame::link("WOAF", url);
        tag.add_frame(frame);
    }
    let mut picture_data: Vec<u8> = Vec::new();
    if let Some(img) = img {
        if let Err(e) = img.write_to(
            &mut Cursor::new(&mut picture_data),
            ImageOutputFormat::Jpeg(90),
        ) {
            println!("Error occured while writing image to buf: {:?}", e)
        } else {
            let picture = Picture {
                mime_type: "image/jpeg".to_string(),
                picture_type: id3::frame::PictureType::CoverFront,
                description: "A picture".to_string(),
                data: picture_data,
            };
            tag.add_frame(picture);
        }
    }
    tag.write_to_path(song.path, id3::Version::Id3v22)
}

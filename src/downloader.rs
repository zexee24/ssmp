use std::{
    collections::hash_map::DefaultHasher,
    fs,
    hash::{Hash, Hasher},
    io::Cursor,
    path::PathBuf,
    process::Command,
    str::FromStr,
};

use id3::{frame::Picture, Frame, Tag, TagLike};
use image::{DynamicImage, EncodableLayout, ImageOutputFormat};
use youtube_dl::YoutubeDl;

use crate::{conf::Configuration, format::Format, song::Song};

pub(crate) async fn download_dlp(url: String) -> Result<Song, String> {
    let conf = Configuration::get_conf();
    let mut fldr = Configuration::get_conf().owned_path;
    let mut hash = DefaultHasher::new();
    url.clone().hash(&mut hash);
    let tfn = format!("{}.temp", hash.finish());
    let data = YoutubeDl::new(&url)
        .socket_timeout("15")
        .youtube_dl_path(conf.ytdlp_path)
        .download(true)
        .format("ba")
        .output_template(&tfn)
        .output_directory(fldr.to_str().unwrap())
        .run_async()
        .await
        .map_err(|e| format!("failed for {}", e))?;
    let d = data.into_single_video().ok_or("Tried a playlist")?;
    let file_name = gen_filename(&d.title);
    let artist = match d.artist {
        Some(a) => Some(a),
        None => d.uploader,
    };
    fldr.push(PathBuf::from_str(&tfn).unwrap());
    let p = change_format_and_name_better(&file_name, fldr).unwrap();
    let s = Song {
        name: d.title,
        artist,
        url: Some(url),
        path: p,
        format: Format::MP3,
    };
    let mut img = None;
    let thumbnail = d.thumbnails;
    if let Some(tnv) = thumbnail {
        if let Some(t) = tnv.iter().max_by_key(|t| t.filesize.unwrap_or(0)) {
            if let Some(u) = &t.url {
                img = get_image(u.to_string()).await;
            }
        }
    }
    if let Err(e) = set_metadata(s.clone(), img) {
        println!("Error when writing metadata: {:?}", e)
    }
    Ok(s)
}

#[cfg(test)]
mod tests {

    use super::download_dlp;
    use super::*;
    #[tokio::test]
    #[ignore = "It downloads"]
    async fn test_dlp() {
        download_dlp("https://www.youtube.com/watch?v=Uk8sAsB25vk".to_string())
            .await
            .unwrap();
    }

    #[test]
    fn test_filename() {
        let new_name = gen_filename("Heilutaan / Eurobeat Remix");
        assert_eq!(new_name, "heilutaan - eurobeat remix")
    }
}

// TODO: Use Opus instead of mp3
pub(crate) fn change_format_and_name_better(name: &str, path: PathBuf) -> Result<PathBuf, String> {
    let new_file_name = gen_filename(name) + &Format::MP3.filetype_to_extension().unwrap();
    let mut new_loc = Configuration::get_conf().owned_path;
    new_loc.push(new_file_name);

    Command::new("ffmpeg")
        .args(["-i", path.to_str().unwrap(), new_loc.to_str().unwrap()])
        .output()
        .expect("Failed the command");
    fs::remove_file(path).unwrap();
    Ok(new_loc)
}

fn gen_filename(name: &str) -> String {
    name.replace(['/', '\\'], "-")
        .replace([':', '.', '!', '?', '\"', '\''], "")
        .to_ascii_lowercase()
}

async fn get_image(url: String) -> Option<DynamicImage> {
    let resp = reqwest::get(url).await.ok()?;
    let bytes = resp.bytes().await.unwrap();
    image::io::Reader::new(Cursor::new(bytes.as_bytes()))
        .with_guessed_format()
        .ok()?
        .decode()
        .ok()
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

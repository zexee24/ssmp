use std::{
    env, fs,
    io::{self, Write},
    path::PathBuf,
    process::Command,
};

use rustube::{blocking::Video, Id};
use symphonia::{
    core::{
        audio::{RawSampleBuffer, SampleBuffer},
        codecs::{DecoderOptions, CODEC_TYPE_NULL},
        errors::Error,
        formats::FormatOptions,
        io::MediaSourceStream,
        meta::MetadataOptions,
        probe::Hint,
    },
    default,
};

pub(crate) fn download(url: String) {
    let id = Id::from_raw(url.as_str()).unwrap();
    let video = Video::from_id((move || id.as_owned())()).unwrap();
    let best_video = video
        .streams()
        .iter()
        .filter(|stream| stream.includes_audio_track)
        .min_by_key(|stream| stream.quality_label)
        .unwrap();
    let path = best_video
        .blocking_download_to_dir(env::current_dir().unwrap().join("songs/"))
        .unwrap();
    let name: String = video.title().to_string();
    //change_format_and_name(path.clone(), name);
    change_format_and_name_better(
        path.clone()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string(),
        name,
    );
}

pub(crate) fn change_format_and_name_better(from: String, to: String) {
    Command::new("ffmpeg")
        .args([
            "-i",
            &("songs/".to_owned() + &from),
            &("songs/".to_owned() + &fix_file_name(to) + ".mp3"),
        ])
        .output()
        .expect("Failed the command");
    fs::remove_file("songs/".to_owned() + &from).unwrap();
}

fn fix_file_name(name: String) -> String {
    return name
        .replace("/", "-")
        .replace("\\", "-")
        .replace(":", "")
        .replace(".", "");
}

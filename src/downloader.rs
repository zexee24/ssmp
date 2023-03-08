use std::{env, fs, process::Command};

use rustube::{blocking::Video, Id};

pub(crate) fn download(url: String) -> Result<String, String> {
    let id = Id::from_raw(url.as_str());
    return match id {
        Ok(id) => {
            let video = Video::from_id((move || id.as_owned())()).unwrap();
            let best_video = video
                .streams()
                .iter()
                .filter(|stream| stream.includes_audio_track)
                .min_by_key(|stream| stream.quality_label)
                .ok_or("Error in getting stream")?;
            let path = best_video
                .blocking_download_to_dir(env::current_dir().unwrap().join("songs/"))
                .map_err(|e| format!("Failed the download: {e}"))?;
            let name: String = video.title().to_string();
            //change_format_and_name(path.clone(), name);
            let file_name = change_format_and_name_better(
                path.clone()
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_string(),
                name,
            );
            Ok(file_name)
        }
        Err(e) => Err(format!("Unable to get video id: {e}")),
    };
}

pub(crate) fn change_format_and_name_better(from: String, to: String) -> String {
    let new_file_name = "songs/".to_owned() + &fix_file_name(to) + ".mp3";

    Command::new("ffmpeg")
        .args(["-i", &("songs/".to_owned() + &from), &new_file_name])
        .output()
        .expect("Failed the command");
    fs::remove_file("songs/".to_owned() + &from).unwrap();
    return new_file_name;
}

fn fix_file_name(name: String) -> String {
    return name
        .replace("/", "-")
        .replace("\\", "-")
        .replace(":", "")
        .replace(".", "")
        .replace("!", "")
        .replace("?", "");
}

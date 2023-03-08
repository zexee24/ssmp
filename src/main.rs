pub mod commands;
pub mod downloader;
pub mod player_state;
pub mod song;

use commands::PlayerMessage;
use downloader::change_format_and_name_better;
use rodio::{OutputStream, Sink, Source};
use serde_json::Value;
use sha256::digest;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::ffi::OsString;
use std::io::prelude::*;
use std::io::{stdin, BufRead, BufReader};
use std::net::TcpStream;
use std::process::exit;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::Sender;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;
use std::*;
use sync::atomic::Ordering::*;

use crate::player_state::PlayerState;
use crate::song::Song;

static SUCCESS: &str = "HTTP/1.1 200 Ok \r\n\r\n";
static FORBIDDEN: &str = "HTTP/1.1 401 Unauthorized \r\n\r\n";
static CONF_PATH: &str = "conf.json";

fn main() {
    println!("Starting player");
    let (ps, pr) = mpsc::channel::<commands::PlayerMessage>();
    let status: Arc<Mutex<PlayerState>> = Arc::new(Mutex::new(PlayerState {
        now_playing: None,
        queue: VecDeque::new(),
        volume: 1.0,
        speed: 1.0,
        paused: true,
        source_duration: None,
    }));
    let status_sender = status.clone();
    let stop_remote: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));

    std::thread::spawn(move || {
        let mut queue: VecDeque<Song> = VecDeque::new();
        let mut now_playing: Option<Song> = None;
        let mut current_duration: Option<Duration> = None;
        let (_stream, stream_handle) = OutputStream::try_default().unwrap();
        let sink = Sink::try_new(&stream_handle).unwrap();
        let default_volume = match fs::read_to_string(CONF_PATH) {
            Ok(file) => match serde_json::from_str::<Value>(&file) {
                Ok(json) => json["Default-Volume"].as_f64().unwrap_or(1.0) as f32,
                Err(e) => {
                    println!("Failed to get default for {:?}", e);
                    1.0
                }
            },
            Err(_) => 1.0,
        };
        sink.set_volume(default_volume);

        loop {
            // Add the next song to the queue if the queue is empty
            if sink.empty() && queue.len() > 0 {
                let song = queue.pop_front();
                if let Some(song) = song {
                    let source = song.create_source();
                    match source {
                        Ok(source) => {
                            current_duration = source.total_duration();
                            now_playing = Some(song);
                            sink.append(source);
                        }
                        Err(e) => println!("Error reached when appending: {:#?}", e),
                    }
                }
            } else if sink.empty() && queue.len() == 0 {
                now_playing = None;
                current_duration = None;
            }

            // Update state
            let mut editable = status_sender.lock().unwrap();
            editable.now_playing = now_playing.clone();
            editable.queue = queue.clone();
            editable.speed = sink.speed();
            editable.volume = sink.volume();
            editable.paused = sink.is_paused();
            editable.source_duration = current_duration;
            drop(editable);

            // Handle a message if one is recieved
            let message_or_error = pr.try_recv();
            match message_or_error {
                Ok(message) => match message {
                    PlayerMessage::Stop => {
                        queue.clear();
                        sink.stop();
                    }
                    PlayerMessage::Pause => sink.pause(),
                    PlayerMessage::Play => sink.play(),
                    PlayerMessage::Volume(v) => sink.set_volume(v),
                    PlayerMessage::Skip(list) => {
                        let mut sorted = list.clone();
                        sorted.sort_by(|a, b| b.cmp(a));
                        for index in sorted.as_ref() {
                            match index {
                                0 => sink.stop(),
                                _ => {
                                    queue.remove(*index - 1);
                                }
                            }
                        }
                    }
                    PlayerMessage::Add(s) => {
                        let song = Song {
                            name: s.clone(),
                            artist: "Artist Unknown".to_string(),
                            url: "Url Unknown".to_string(),
                            path: OsString::from(s),
                        };
                        queue.push_back(song);
                    }
                    PlayerMessage::Clear => queue.clear(),
                    PlayerMessage::Speed(s) => sink.set_speed(s),
                    PlayerMessage::ReOrder(origin, dest) => {
                        let elem = queue.remove(origin);
                        if let Some(song) = elem {
                            queue.insert(dest, song)
                        }
                    }
                },
                Err(_) => (),
            }
        }
    });

    for command in stdin().lock().lines() {
        match command {
            Ok(command) => handle_command(
                command.trim(),
                ps.clone(),
                stop_remote.clone(),
                status.clone(),
            ),
            Err(_) => println!("Error handling input stream"),
        }
    }
    exit_program()
}

fn handle_command(
    command: &str,
    ps: Sender<PlayerMessage>,
    stop_remote: Arc<AtomicBool>,
    state: Arc<Mutex<PlayerState>>,
) {
    let (command, value) = command.split_once(" ").unwrap_or((command, ""));
    match command {
        "list" => println!("{:?}", list_songs()),
        "volume" => ps
            .send(PlayerMessage::Volume(value.parse::<f32>().unwrap_or(1.0)))
            .unwrap(),
        "add" => ps
            .send(PlayerMessage::Add("songs/".to_owned() + &value.to_string()))
            .unwrap(),
        "play" | "continue" => ps.send(PlayerMessage::Play).unwrap(),
        "stop" => ps.send(PlayerMessage::Stop).unwrap(),
        "clear" => ps.send(PlayerMessage::Clear).unwrap(),
        "pause" => ps.send(PlayerMessage::Pause).unwrap(),
        "exit" => exit_program(),
        "speed" => ps
            .send(PlayerMessage::Speed((value.parse::<f32>()).unwrap_or(1.0)))
            .unwrap(),
        "remote" => match value {
            "start" => {
                start_remote(ps.clone(), stop_remote.clone(), state.clone());
                stop_remote.store(false, SeqCst)
            }
            "stop" => stop_remote.store(true, SeqCst),
            _ => println!("Unknown subcommand of'remote'"),
        },
        "now" | "nowplaying" | "current" | "np" => {
            println!("{:?}", state.lock().unwrap().now_playing)
        }
        "queue" | "que" => {
            let queue = &state.lock().unwrap().queue;
            for song in queue {
                println!("{:?}", song.name)
            }
        }
        "status" => {
            println!("{:#?}", state.lock().unwrap())
        }
        "move" | "reorder" => {
            let t = value.split_once(" ").unwrap_or(("0", "0"));
            let (from, to) = (
                t.0.parse::<usize>().unwrap_or(0),
                t.1.parse::<usize>().unwrap_or(0),
            );
            ps.send(PlayerMessage::ReOrder(from, to)).unwrap();
        }
        "skip" => {
            let mut list: Vec<usize> = Vec::new();
            for arg in value.split(" ") {
                let num = arg.parse::<usize>();
                if let Ok(num) = num {
                    list.push(num)
                }
            }
            ps.send(PlayerMessage::Skip(list.into())).unwrap();
        }
        "downloadb" | "db" => {
            let result = downloader::download(value.to_string());
            if let Err(e) = result {
                println!("{e}");
            }
        }
        "download" | "d" => {
            let val = value.clone().to_string();
            thread::spawn(|| {
                let result = downloader::download(val);
                if let Err(e) = result {
                    println!("{e}");
                }
            });
        }
        "download-add" | "da" => {
            let pst = ps.clone();
            let val = value.clone().to_string();
            thread::spawn(move || {
                let result = downloader::download(val);
                match result {
                    Err(e) => println!("{e}"),
                    Ok(file_name) => pst.send(PlayerMessage::Add(file_name)).unwrap(),
                }
            });
        }
        "convert" => {
            change_format_and_name_better(value.to_string(), "test".to_string());
        }
        _ => println!("Unknown command"),
    }
}

fn list_songs() -> Vec<String> {
    let mut song_list: Vec<String> = Vec::new();
    let dir = fs::read_dir("songs/").unwrap();
    for file in dir {
        song_list.push(file.unwrap().file_name().to_str().unwrap().to_string());
    }
    return song_list;
}

fn start_remote(
    ps: Sender<PlayerMessage>,
    stop_remote: Arc<AtomicBool>,
    state: Arc<Mutex<PlayerState>>,
) {
    thread::spawn(move || {
        let addr = "192.168.2.116:8008";
        let listener = std::net::TcpListener::bind(addr).unwrap();
        println!("Remote started on: {}", addr);
        for stream in listener.incoming() {
            //Now only ends the remote when someone makes a request
            if stop_remote.load(SeqCst) {
                break;
            }
            println!("Connection established");
            if let Some(stream) = stream.ok() {
                handle_stream(stream, ps.clone(), state.clone());
            }
        }
        println!("Remote ended")
    });
}

fn handle_stream(mut stream: TcpStream, ps: Sender<PlayerMessage>, state: Arc<Mutex<PlayerState>>) {
    let mut reader = BufReader::new(&mut stream);

    let mut header_map: HashMap<String, String> = HashMap::new();
    let mut request = String::new();
    reader.read_line(&mut request).unwrap();
    loop {
        let mut buffer: String = String::new();
        let result = reader.read_line(&mut buffer);
        match result {
            Ok(0) => break,
            Ok(_) => match buffer.split_once(" ").unwrap_or(("", "")) {
                ("", "") => break,
                (k, v) => {
                    header_map.insert(k.trim().to_owned(), v.trim().to_owned());
                    ()
                }
            },
            Err(e) => {
                println!("Error {:?} reached", e);
                break;
            }
        }
    }

    let authorized: bool = match fs::read_to_string(CONF_PATH) {
        Ok(conf) => {
            if let Ok(json) = serde_json::from_str::<Value>(conf.as_str()) {
                let stored = json["Access-Key"].as_str().unwrap_or("");
                let recieved = digest(
                    header_map
                        .get("Access-Key:")
                        .unwrap_or(&"".to_string())
                        .as_str(),
                );
                stored == recieved
            } else {
                false
            }
        }
        Err(_) => false,
    };

    let mut body = String::new();

    match header_map.get("Content-Length:") {
        Some(v) => {
            let mut buffer = vec![0u8; v.parse::<usize>().unwrap_or(0)];
            reader.read_exact(&mut buffer).unwrap();
            body = String::from_utf8(buffer).unwrap_or("".to_string());
        }
        None => (),
    }

    if authorized {
        let response = match request.trim() {
            "GET / HTTP/1.1" => SUCCESS.to_string(),
            "POST /pause HTTP/1.1" => {
                ps.send(PlayerMessage::Pause).unwrap();
                SUCCESS.to_string()
            }
            "POST /play HTTP/1.1" => {
                ps.send(PlayerMessage::Play).unwrap();
                SUCCESS.to_string()
            }
            "POST /skip HTTP/1.1" => {
                let mut list = Vec::new();
                for line in body.lines() {
                    if let Ok(n) = line.parse::<usize>() {
                        list.push(n)
                    }
                }
                ps.send(PlayerMessage::Skip(list.into())).unwrap();
                SUCCESS.to_string()
            }
            "POST /add HTTP/1.1" => {
                for line in body.lines() {
                    ps.send(PlayerMessage::Add(line.to_string())).unwrap();
                }
                SUCCESS.to_string()
            }
            "POST /download HTTP/1.1" => {
                for line in body.lines() {
                    let l = line.clone().to_owned();
                    thread::spawn(move || {
                        let result = downloader::download(l);
                        if let Err(e) = result {
                            println!("{e}");
                        }
                    });
                }
                SUCCESS.to_string()
            }
            "POST /download/add HTTP/1.1" => {
                for line in body.lines() {
                    let l = line.clone().to_owned();
                    let pst = ps.clone();
                    thread::spawn(move || {
                        let result = downloader::download(l);
                        match result {
                            Err(e) => println!("{e}"),
                            Ok(file_name) => pst.send(PlayerMessage::Add(file_name)).unwrap(),
                        }
                    });
                }
                SUCCESS.to_string()
            }
            "POST /volume HTTP/1.1" => {
                ps.send(PlayerMessage::Volume(body.parse::<f32>().unwrap_or(1.0)))
                    .unwrap();
                SUCCESS.to_string()
            }
            "POST /speed HTTP/1.1" => {
                ps.send(PlayerMessage::Speed(body.parse::<f32>().unwrap_or(1.0)))
                    .unwrap();
                SUCCESS.to_string()
            }
            "GET /list HTTP/1.1" => {
                let list = list_songs();
                let json = serde_json::to_string(&list).unwrap();
                json_to_https(json)
            }
            "GET /status HTTP/1.1" => {
                let json = serde_json::to_string(&state).unwrap();
                json_to_https(json)
            }
            _ => "HTTP/1.1 404 NOT FOUND\r\n\r\n".to_string(),
        };
        stream.write_all(response.as_bytes()).unwrap();
    } else {
        let response = match request.trim() {
            "GET / HTTP/1.1" => SUCCESS.to_string(),
            "POST /pause HTTP/1.1" => FORBIDDEN.to_string(),
            "POST /play HTTP/1.1" => FORBIDDEN.to_string(),
            "POST /skip HTTP/1.1" => FORBIDDEN.to_string(),
            "POST /add HTTP/1.1" => FORBIDDEN.to_string(),
            "POST /volume HTTP/1.1" => FORBIDDEN.to_string(),
            "POST /speed HTTP/1.1" => FORBIDDEN.to_string(),
            "GET /list HTTP/1.1" => {
                let list = list_songs();
                let json = serde_json::to_string(&list).unwrap();
                json_to_https(json)
            }
            "GET /status HTTP/1.1" => {
                let json = serde_json::to_string(&state).unwrap();
                json_to_https(json)
            }
            "POST /download/add HTTP/1.1" => FORBIDDEN.to_string(),
            "POST /download HTTP/1.1" => FORBIDDEN.to_string(),
            _ => "HTTP/1.1 404 NOT FOUND\r\n\r\n".to_string(),
        };
        stream.write_all(response.as_bytes()).unwrap();
    }
}

fn json_to_https(json: String) -> String {
    let len = json.as_bytes().len();
    return format!(
        "HTTP/1.1 200 Ok\r\nContent-Type: application/json\r\nContent-Length: {len}\r\n\r\n{json}"
    );
}

fn exit_program() {
    println!("Exitting");
    exit(0)
}

use std::{
    collections::HashMap,
    fs,
    io::BufReader,
    net::TcpStream,
    sync::{atomic::AtomicBool, mpsc::Sender, Arc, Mutex},
    thread,
};

use serde_json::Value;
use sha256::digest;

use crate::{
    commands::PlayerMessage, downloader, list_songs, player_state::PlayerState, CONF_PATH,
};
use std::io::prelude::*;
use std::io::BufRead;
use std::io::Read;
use std::sync::atomic::Ordering::SeqCst;
use std::*;

static SUCCESS: &str = "HTTP/1.1 200 Ok \r\n\r\n";
static FORBIDDEN: &str = "HTTP/1.1 401 Unauthorized \r\n\r\n";
pub fn start_remote(
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

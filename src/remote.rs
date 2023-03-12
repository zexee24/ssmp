use std::{
    collections::HashMap,
    io::BufReader,
    net::TcpStream,
    sync::{atomic::AtomicBool, mpsc::Sender, Arc, Mutex},
    thread,
};

#[cfg(test)]
use http_bytes::http::{header, HeaderValue};

use sha256::digest;

use crate::{
    commands::PlayerMessage, downloader, list_songs, player_state::PlayerState, song::Song,
};
use std::io::prelude::*;
use std::io::BufRead;
use std::io::Read;
use std::sync::atomic::Ordering::SeqCst;
use std::*;

use crate::conf::*;

static SUCCESS: &str = "HTTP/1.1 200 Ok \r\n\r\n";
static FORBIDDEN: &str = "HTTP/1.1 401 Unauthorized \r\n\r\n";
pub fn start_remote(
    ps: Sender<PlayerMessage>,
    stop_remote: Arc<AtomicBool>,
    state: Arc<Mutex<PlayerState>>,
    addresses: Vec<String>,
) {
    for addr in addresses {
        let psx = ps.clone();
        let statex = state.clone();
        let srx = stop_remote.clone();
        thread::spawn(move || {
            match std::net::TcpListener::bind(addr.clone()) {
                Ok(listener) => {
                    println!("Remote started on: {}", addr);
                    for stream in listener.incoming() {
                        //Now only ends the remote when someone makes a request
                        if srx.clone().load(SeqCst) {
                            break;
                        }
                        println!("Connection established");
                        if let Ok(stream) = stream {
                            handle_stream(stream, psx.clone(), statex.clone());
                        }
                    }
                    println!("Remote ended")
                }
                Err(e) => {
                    println!("Failed to bind {} cause of {}", addr, e)
                }
            }
        });
    }
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
            Ok(_) => match buffer.split_once(' ').unwrap_or(("", "")) {
                ("", "") => break,
                (k, v) => {
                    header_map.insert(k.trim().to_owned(), v.trim().to_owned());
                }
            },
            Err(e) => {
                println!("Error {:?} reached", e);
                break;
            }
        }
    }

    // This line is dumb
    let authorized: bool = Configuration::get_conf().access_key
        == digest(
            header_map
                .get("Access-Key:")
                .unwrap_or(&"".to_string())
                .as_str(),
        );

    let mut body = String::new();

    if let Some(v) = header_map.get("Content-Length:") {
        let mut buffer = vec![0u8; v.parse::<usize>().unwrap_or(0)];
        reader.read_exact(&mut buffer).unwrap();
        body = String::from_utf8(buffer).unwrap_or("".to_string());
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
                    let songopt = Song::from_string(line.to_owned());
                    if let Some(song) = songopt {
                        ps.send(PlayerMessage::Add(song)).unwrap();
                    }
                }
                SUCCESS.to_string()
            }
            "POST /download HTTP/1.1" => {
                for line in body.lines() {
                    let l = line.to_owned();
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
                    let l = line.to_owned();
                    let pst = ps.clone();
                    thread::spawn(move || {
                        let result = downloader::download(l);
                        match result {
                            Err(e) => println!("{e}"),
                            Ok(song) => pst.send(PlayerMessage::Add(song)).unwrap(),
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
                json_to_http(json)
            }
            "GET /status HTTP/1.1" => {
                let json = serde_json::to_string(&state).unwrap();
                json_to_http(json)
            }
            "GET /picture HTTP/1.1" => {
                let list = list_songs();
                let mut song_img_list: Vec<(Song, Vec<u8>)> = Vec::new();
                for line in body.lines() {
                    if let Some(song) = list
                        .iter()
                        .find(|s| s.name == line || s.url == Some(line.to_owned()))
                    {
                        if let Some(img) = song.get_image() {
                            song_img_list.push((song.clone(), img));
                        }
                    }
                }
                stream
                    .write_all(&image_list_to_http(song_img_list))
                    .unwrap();
                SUCCESS.to_string()
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
                json_to_http(json)
            }
            "GET /status HTTP/1.1" => {
                let json = serde_json::to_string(&state).unwrap();
                json_to_http(json)
            }
            "POST /download/add HTTP/1.1" => FORBIDDEN.to_string(),
            "POST /download HTTP/1.1" => FORBIDDEN.to_string(),
            _ => "HTTP/1.1 404 NOT FOUND\r\n\r\n".to_string(),
        };
        stream.write_all(response.as_bytes()).unwrap();
    }
}

fn json_to_http(json: String) -> String {
    let len = json.as_bytes().len();
    format!(
        "HTTP/1.1 200 Ok\r\nContent-Type: application/json\r\nContent-Length: {len}\r\n\r\n{json}"
    )
}

fn image_list_to_http(list: Vec<(Song, Vec<u8>)>) -> Vec<u8> {
    let head = "HTTP/1.1 200 Ok\r\nContent-Type: multipart/form-data;boundary=\"b\"";
    let mut body_raw: Vec<u8> = Vec::new();
    body_raw.append(&mut head.as_bytes().to_vec());
    for (song, mut image) in list {
        body_raw.append(&mut "\r\n\r\n".as_bytes().to_vec());
        body_raw.append(&mut "--b\r\n".as_bytes().to_vec());
        body_raw.append(
            &mut format!(
                "Content-Type: image/jpeg; Content-Disposition: form-data; name={}\r\n\r\n",
                song.name
            )
            .as_bytes()
            .to_vec(),
        );
        body_raw.append(&mut image);
    }
    body_raw
}

#[test]
fn test_image_to_http() {
    let song = Song::from_string("I Will Give You My All 2017".to_string()).unwrap();
    let img = song.get_image().unwrap();
    let vec = vec![(song, img.clone())];
    let http = image_list_to_http(vec);
    let mut headers_buffer = vec![http_bytes::EMPTY_HEADER; 64];
    let (r, mut b) = http_bytes::parse_response_header(&http, &mut headers_buffer)
        .unwrap()
        .unwrap();
    assert!(r.status() == http_bytes::http::StatusCode::OK);
    assert_eq!(
        r.headers().get(header::CONTENT_TYPE),
        Some(&HeaderValue::from_str("multipart/form-data;boundary=\"b\"").unwrap())
    );

    b.read_line(&mut String::new()).unwrap();
    b.read_line(&mut String::new()).unwrap();
    b.read_line(&mut String::new()).unwrap();
    let mut img_from_body = Vec::new();
    loop {
        let mut buf = Vec::new();
        let num = b.read_until(0x0A, &mut buf).unwrap();
        if buf != b"--b\r\n" {
            img_from_body.append(&mut buf)
        } else {
            break;
        }
        if num == 0 {
            break;
        }
    }
    assert_eq!(img_from_body, img)
}

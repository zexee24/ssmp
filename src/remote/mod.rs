use core::sync;
use std::{
    collections::HashMap,
    io::{BufRead, Read, Write},
    sync::{atomic::AtomicBool, Arc, Mutex},
    thread::{self},
    time::Duration,
};
pub(crate) mod auth;
use base64::{engine, Engine};

use itertools::Itertools;
use sha256::digest;
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
    select,
    sync::mpsc::Sender,
};

use crate::{
    commands::PlayerMessage,
    downloader, list_songs,
    player_state::PlayerState,
    song::{Song, SongWithImage},
};
use std::sync::atomic::Ordering::SeqCst;
use std::*;
use tokio::task;

use crate::conf::*;

use self::auth::Permission;

static SUCCESS: &str = "HTTP/1.1 200 Ok \r\n\r\n";
static FORBIDDEN: &str = "HTTP/1.1 401 Unauthorized \r\n\r\n";

pub struct RemoteHandler {
    ps: Sender<PlayerMessage>,
    state: Arc<Mutex<PlayerState>>,
    address_listeners: Vec<AddressListener>,
}

struct AddressListener {
    address: String,
    stop_handle: Arc<AtomicBool>,
    ps: Sender<PlayerMessage>,
}

struct Request {
    method: String,
    protocol: String,
    headers: HashMap<String, String>,
    permissions: Vec<Permission>,
    body: Option<String>,
}

enum ResponceTypes<'a> {
    Success(Option<&'a str>),
    Forbidden,
    BadRequest(Option<&'a str>),
    NotFound,
}

macro_rules! check_permissions {
    ($a: expr, $b: expr) => {{
        if let Err(e) = $b.check_permissions($a) {
            return e;
        };
    }};
}

macro_rules! send_until_succ {
    ($a: expr, $b: expr) => {
        while let Err(e) = $a.send($b).await {
            println!("{:?}", e);
        }
    };
}

macro_rules! require_body {
    ($b: expr) => {
        match $b {
            Some(b) => b,
            None => {
                return ResponceTypes::BadRequest(Some("This request requires a body"))
                    .get_responce()
            }
        }
    };
}

impl ResponceTypes<'_> {
    fn get_responce(&self) -> String {
        match self {
            ResponceTypes::Success(d) => {
                format!("HTTP/1.1 200 Ok \r\n\r\n{}", d.unwrap_or(""))
            }
            ResponceTypes::Forbidden => "HTTP/1.1 401 Unauthorized \r\n\r\n".to_string(),
            ResponceTypes::BadRequest(s) => {
                format!("HTTP/1.1 402 Bad request \r\n\r\n{}", s.unwrap_or(""))
            }
            ResponceTypes::NotFound => "HTTP/1.1 404 Not found \r\n\r\n".to_string(),
        }
    }
}

impl Request {
    pub fn check_permissions(&self, required_permissions: &[Permission]) -> Result<(), String> {
        if self
            .permissions
            .iter()
            .all(|p| required_permissions.contains(p))
        {
            return Err(ResponceTypes::Forbidden.get_responce());
        }
        Ok(())
    }
}

impl AddressListener {
    async fn new(
        address: String,
        stop_handle: Arc<AtomicBool>,
        ps: Sender<PlayerMessage>,
    ) -> Result<AddressListener, String> {
        let adrl = AddressListener {
            address: address.to_string(),
            stop_handle,
            ps,
        };
        match adrl.start().await {
            Ok(_) => Ok(adrl),
            Err(e) => Err(format!("Unable to create listener for {e}")),
        }
    }

    async fn start(&self) -> Result<(), std::io::Error> {
        let lister = TcpListener::bind(self.address.as_str()).await?;
        let sh = self.stop_handle.clone();
        let psx = self.ps.clone();
        tokio::spawn(async move {
            let handle = task::spawn(async move {
                println!("Connection Accepted:");
                loop {
                    let (s, _a) = lister.accept().await.unwrap();
                    println!("Connection Accepted:");
                    Self::handle_request(s, psx.clone()).await;
                }
            });
            let int = tokio::spawn(async move {
                loop {
                    match sh.load(SeqCst) {
                        true => break,
                        false => {
                            tokio::time::sleep(Duration::from_millis(500)).await;
                        }
                    }
                }
            });
            select! {
                _ = handle => (),
                _ = int => (),
            }
        });
        Ok(())
    }

    async fn handle_request(mut s: tokio::net::TcpStream, ps: Sender<PlayerMessage>) {
        let request = Self::parse_request(BufReader::new(&mut s)).await;
        match request {
            Ok(r) => match r.protocol.trim() {
                "HTTP/1.1" => {
                    s.write_all(Self::handle_http1_1(r, ps).await.as_bytes())
                        .await
                        .unwrap();
                }
                _ => s
                    .write_all(
                        ResponceTypes::BadRequest(Some("Unsupported protocol"))
                            .get_responce()
                            .as_bytes(),
                    )
                    .await
                    .unwrap(),
            },
            Err(e) => {
                s.write_all(
                    ResponceTypes::BadRequest(Some(&e))
                        .get_responce()
                        .as_bytes(),
                )
                .await
                .unwrap();
            }
        }
    }

    async fn handle_http1_1(r: Request, ps: Sender<PlayerMessage>) -> String {
        match r.method.as_str() {
            "GET /" => {
                check_permissions!(&[Permission::Info], r);
                ResponceTypes::Success(None).get_responce()
            }
            "POST /play" => {
                check_permissions!(&[Permission::PlayPause], r);
                send_until_succ!(ps, PlayerMessage::Play);
                ResponceTypes::Success(None).get_responce()
            }
            "POST /pause" => {
                check_permissions!(&[Permission::PlayPause], r);
                send_until_succ!(ps, PlayerMessage::Pause);
                ResponceTypes::Success(None).get_responce()
            }
            "POST /skip" => {
                check_permissions!(&[Permission::Seek], r);
                let body = require_body!(r.body);
                let mut l = vec![];
                for line in body.lines() {
                    if let Ok(n) = line.parse::<usize>() {
                        l.push(n)
                    }
                }
                send_until_succ!(ps, PlayerMessage::Skip(l.clone().into()));
                ResponceTypes::Success(None).get_responce()
            }
            "POST /add" => {
                check_permissions!(&[Permission::Add], r);
                let body = require_body!(r.body);
                for line in body.lines() {
                    if let Some(song) = Song::from_string(line.to_owned()) {
                        send_until_succ!(ps, PlayerMessage::Add(song.clone()));
                    }
                }
                ResponceTypes::Success(None).get_responce()
            }
            "POST /download" => {
                check_permissions!(&[Permission::Download], r);
                let body = require_body!(r.body);
                //for line in body {}
                ResponceTypes::Success(None).get_responce()
            }
            _ => ResponceTypes::NotFound.get_responce(),
        }
    }

    async fn parse_request(mut buf: BufReader<&mut TcpStream>) -> Result<Request, String> {
        let mut headers: HashMap<String, String> = HashMap::new();
        let mut m = String::new();
        buf.read_line(&mut m).await.map_err(|_| "Failed a read")?;
        let (method, protocol) = Self::method_and_procol_from_line(m)?;
        loop {
            let mut st = String::new();
            buf.read_line(&mut st).await.map_err(|_| "Failed a read")?;
            if st != "\r\n" {
                break;
            }
            if let Some((k, v)) = st.split_once(':') {
                headers.insert(k.to_string(), v.to_string());
            }
        }
        let key = match headers.get("Key") {
            Some(k) => k,
            None => "",
        };
        let permissions = Self::get_permissions(key);

        return match headers.get("Content-Length") {
            Some(l) => {
                let mut buffer = vec![
                    0u8;
                    l.parse::<usize>().map_err(|_| {
                        "Unable to parse Content-Length".to_string()
                    })?
                ];
                buf.read_exact(&mut buffer)
                    .await
                    .map_err(|_| "Unable to read promisec body".to_string())?;
                let body = String::from_utf8(buffer)
                    .map_err(|_| "Unable to parse body to a String".to_string())?;
                Ok(Request {
                    method,
                    protocol,
                    headers,
                    body: Some(body),
                    permissions,
                })
            }
            None => Ok(Request {
                method,
                protocol,
                headers,
                body: None,
                permissions,
            }),
        };
    }

    fn method_and_procol_from_line(line: String) -> Result<(String, String), String> {
        let (f, s, t) = line
            .split(' ')
            .collect_tuple()
            .ok_or("Failed to process method")?;
        Ok((format!("{} {}", f, s), t.to_string()))
    }

    fn get_permissions(key: &str) -> Vec<Permission> {
        let conf = Configuration::get_conf();
        for k in conf.keys {
            if k.key == key {
                return k.permissions;
            }
        }
        vec![]
    }

    fn stop(&self) {
        self.stop_handle.store(true, SeqCst);
    }
}

impl RemoteHandler {
    pub fn list_listeners(&self) -> Vec<&str> {
        self.address_listeners
            .iter()
            .map(|a| a.address.as_str())
            .collect()
    }

    pub fn stop_listener(&self, addrs: String) -> Result<(), String> {
        match self.address_listeners.iter().find(|a| a.address == addrs) {
            Some(listener) => {
                listener.stop();
                Ok(())
            }
            None => Err(format!("Listener for {:?} does no exist", addrs)),
        }
    }

    pub async fn new_listener(&mut self, addrs: String) -> Result<(), String> {
        let a =
            AddressListener::new(addrs, Arc::new(AtomicBool::new(false)), self.ps.clone()).await?;
        self.address_listeners.push(a);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{atomic::AtomicBool, mpsc, Arc},
        time::Duration,
    };

    use serial_test::serial;
    use tokio::sync::mpsc::channel;

    use super::AddressListener;

    async fn create_valid_listener() -> AddressListener {
        let (s, _) = channel(32);
        let a = AddressListener::new(
            "127.0.0.1:8000".to_string(),
            Arc::new(AtomicBool::new(false)),
            s,
        )
        .await;
        assert!(a.is_ok());
        a.unwrap()
    }
    #[tokio::test]
    #[serial]
    async fn test_listener_valid_ip() {
        create_valid_listener().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_listener_invalid_ip() {
        let (s, _) = channel(32);
        let adrl = AddressListener::new(
            "slakhfjaskghak".to_string(),
            Arc::new(AtomicBool::new(false)),
            s.clone(),
        )
        .await;
        assert!(adrl.is_err());
        let adrl = AddressListener::new(
            "195.251.52.14:90".to_string(),
            Arc::new(AtomicBool::new(false)),
            s,
        )
        .await;
        assert!(adrl.is_err())
    }

    #[tokio::test]
    #[serial]
    async fn test_response() {
        let adrl = create_valid_listener().await;
        let ip = format!("http://{}", adrl.address);
        let resp = reqwest::get(ip).await;
        assert!(resp.is_ok());
        resp.unwrap();
    }

    #[tokio::test]
    #[serial]
    #[should_panic]
    async fn test_stopping() {
        let a = create_valid_listener().await;
        a.stop();
        test_response();
    }
    #[test]
    fn test_method_line_correct() {
        assert_eq!(
            AddressListener::method_and_procol_from_line("GET / HTTP/1.1".to_string()),
            Ok(("GET /".to_string(), "HTTP/1.1".to_string()))
        );
    }
    #[tokio::test]
    #[ignore = "Manual test"]
    #[serial]
    async fn test_manual() {
        let adrl = create_valid_listener().await;
        tokio::time::sleep(Duration::from_secs(100)).await;
    }
}

pub fn start_remote(
    ps: std::sync::mpsc::Sender<PlayerMessage>,
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
    println!("Returned");
}

fn handle_stream(
    mut stream: std::net::TcpStream,
    ps: std::sync::mpsc::Sender<PlayerMessage>,
    state: Arc<Mutex<PlayerState>>,
) {
    let mut reader = std::io::BufReader::new(&mut stream);

    let mut header_map: HashMap<String, String> = HashMap::new();
    let mut request = String::new();
    reader.read_line(&mut request);
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
    let authorized: bool = Configuration::get_conf().keys.get(0).unwrap().key
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
                ps.send(PlayerMessage::Pause);
                SUCCESS.to_string()
            }
            "POST /play HTTP/1.1" => {
                ps.send(PlayerMessage::Play);
                SUCCESS.to_string()
            }
            "POST /skip HTTP/1.1" => {
                let mut list = Vec::new();
                for line in body.lines() {
                    if let Ok(n) = line.parse::<usize>() {
                        list.push(n)
                    }
                }
                ps.send(PlayerMessage::Skip(list.into()));
                SUCCESS.to_string()
            }
            "POST /add HTTP/1.1" => {
                for line in body.lines() {
                    let songopt = Song::from_string(line.to_owned());
                    if let Some(song) = songopt {
                        ps.send(PlayerMessage::Add(song));
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
                            Ok(song) =>
                            /*pst.send(PlayerMessage::Add(song))*/
                            {
                                ()
                            }
                        }
                    });
                }
                SUCCESS.to_string()
            }
            "POST /volume HTTP/1.1" => {
                ps.send(PlayerMessage::Volume(body.parse::<f32>().unwrap_or(1.0)));
                SUCCESS.to_string()
            }
            "POST /speed HTTP/1.1" => {
                ps.send(PlayerMessage::Speed(body.parse::<f32>().unwrap_or(1.0)));
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
                let mut song_img_list: Vec<SongWithImage> = Vec::new();
                for line in body.lines() {
                    if let Some(song) = list
                        .iter()
                        .find(|s| s.name == line || s.url == Some(line.to_owned()))
                    {
                        if let Some(img) = song.get_image() {
                            let engine = engine::general_purpose::STANDARD;
                            let image = engine.encode(img);
                            song_img_list.push(SongWithImage {
                                song: song.clone(),
                                image,
                            });
                        }
                    }
                }
                json_to_http(serde_json::to_string(&song_img_list).unwrap())
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

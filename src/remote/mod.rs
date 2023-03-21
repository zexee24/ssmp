use std::{
    collections::HashMap,
    ops::Deref,
    sync::{atomic::AtomicBool, Arc, Mutex},
    time::Duration,
};
pub(crate) mod auth;
use base64::{engine, Engine};

use futures::future::join_all;
use itertools::Itertools;
use sha256::digest;
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
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

pub struct RemoteHandler {
    ps: Sender<PlayerMessage>,
    state: Arc<Mutex<PlayerState>>,
    address_listeners: Vec<AddressListener>,
}

struct AddressListener {
    address: String,
    stop_handle: Arc<AtomicBool>,
    ps: Sender<PlayerMessage>,
    state: Arc<Mutex<PlayerState>>,
}

struct Request {
    method: String,
    protocol: String,
    _headers: HashMap<String, String>,
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
                match d {
                    Some(body) => format!("HTTP/1.1 200 Ok \r\nContent-Type: text/json\r\nContent-Length: {}\r\n\r\n{}", body.as_bytes().len(), body),
                    None => "HTTP/1.1 200 Ok \r\n\r\n".to_owned()
                }
                
            }
            ResponceTypes::Forbidden => "HTTP/1.1 401 Unauthorized \r\n\r\n".to_string(),
            ResponceTypes::BadRequest(s) => {
                match s {
                    Some(body) =>format!("HTTP/1.1 402 Bad request\r\nContent-Type: text/plain\r\nContent-Lenght: {}\r\n\r\n{}", body.as_bytes().len(), body),
                    None => "HTTP/1.1 402 Bad request\r\n\r\n".to_owned()
                }
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
        state: Arc<Mutex<PlayerState>>,
    ) -> Result<AddressListener, String> {
        let adrl = AddressListener {
            address: address.to_string(),
            stop_handle,
            ps,
            state,
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
        let state = self.state.clone();
        tokio::spawn(async move {
            let handle = task::spawn(async move {
                loop {
                    let (s, _a) = lister.accept().await.unwrap();
                    Self::handle_request(s, psx.clone(), state.clone()).await;
                }
            });
            tokio::spawn(async move {
                loop {
                    match sh.load(SeqCst) {
                        true => break,
                        false => {
                            tokio::time::sleep(Duration::from_millis(500)).await;
                        }
                    }
                }
            })
            .await
            .unwrap();
            handle.abort();
        });
        Ok(())
    }

    async fn handle_request(
        mut s: tokio::net::TcpStream,
        ps: Sender<PlayerMessage>,
        state: Arc<Mutex<PlayerState>>,
    ) {
        let request = Self::parse_request(BufReader::new(&mut s)).await;
        match request {
            Ok(r) => match r.protocol.trim() {
                "HTTP/1.1" => {
                    s.write_all(Self::handle_http1_1(r, ps, state).await.as_bytes())
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

    async fn handle_http1_1(
        r: Request,
        ps: Sender<PlayerMessage>,
        state: Arc<Mutex<PlayerState>>,
    ) -> String {
        match r.method.as_str() {
            "GET /" => {
                check_permissions!(&[Permission::Info], r);
                let s = state.lock().unwrap();
                ResponceTypes::Success(Some(&serde_json::to_string(s.deref()).unwrap()))
                    .get_responce()
            }
            "GET /list" => {
                check_permissions!(&[Permission::Info], r);
                let json = serde_json::to_string(&list_songs()).unwrap();
                ResponceTypes::Success(Some(&json)).get_responce()
            }
            "GET /picture" => {
                check_permissions!(&[Permission::Info], r);
                let body = require_body!(r.body);
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
                                image: Some(image),
                            });
                        }
                    }
                }
                ResponceTypes::Success(Some(&serde_json::to_string(&song_img_list).unwrap()))
                    .get_responce()
            }
            "GET /picture/list" => {
                let mut song_img_list: Vec<SongWithImage> = vec![];
                for song in list_songs() {
                    let image = song.get_image().map(|i| {
                        let engine = engine::general_purpose::STANDARD;
                        engine.encode(i)
                    });
                    song_img_list.push(SongWithImage { song, image });
                }
                ResponceTypes::Success(Some(&serde_json::to_string(&song_img_list).unwrap()))
                    .get_responce()
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
            "POST /reorder" => {
                check_permissions!(&[Permission::Seek], r);
                let body = require_body!(r.body);
                for line in body.lines(){
                    if let Some((f,t))=  line.split_once(' ') {
                            if let (Ok(f), Ok(t)) = (f.parse::<usize>(), t.parse::<usize>()) {
                                send_until_succ!(ps,PlayerMessage::ReOrder(f, t))
                            }
                        }
                }
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
                let mut handles = vec![];
                for line in body.lines() {
                    handles.push(downloader::download_dlp(line.to_owned()));
                }
                join_all(handles).await;
                ResponceTypes::Success(None).get_responce()
            }
            "POST /download/add" => {
                check_permissions!(&[Permission::Download], r);
                let body = require_body!(r.body);
                let mut handles = vec![];
                for line in body.lines() {
                    handles.push(Self::download_and_add(line.to_string(), ps.clone()));
                }
                join_all(handles).await;
                ResponceTypes::Success(None).get_responce()
            }
            "POST /volume" => {
                let body = require_body!(r.body);
                match body.parse::<f32>() {
                    Ok(target_volume) => {
                        for p in r.permissions {
                            match p {
                                Permission::VolumeControl((min, max)) => {
                                    if min <= target_volume && target_volume <= max {
                                        send_until_succ!(ps, PlayerMessage::Volume(target_volume));
                                        return ResponceTypes::Success(None).get_responce();
                                    } else {
                                        return ResponceTypes::Forbidden.get_responce();
                                    }
                                }
                                _ => continue,
                            }
                        }
                        ResponceTypes::Forbidden.get_responce()
                    }
                    Err(e) => ResponceTypes::BadRequest(Some(&e.to_string())).get_responce(),
                }
            }
            "POST /speed" => {
                check_permissions!(&[Permission::Seek], r);
                let body = require_body!(r.body);
                match body.parse::<f32>() {
                    Ok(n) => {
                        send_until_succ!(ps, PlayerMessage::Speed(n));
                        ResponceTypes::Success(None).get_responce()
                    }
                    Err(e) => ResponceTypes::BadRequest(Some(&e.to_string())).get_responce(),
                }
            }
            _ => ResponceTypes::NotFound.get_responce(),
        }
    }

    async fn download_and_add(url: String, ps: Sender<PlayerMessage>) -> Result<(), String> {
        let song = downloader::download_dlp(url).await?;
        Ok(send_until_succ!(ps, PlayerMessage::Add(song.clone())))
    }

    async fn parse_request(mut buf: BufReader<&mut TcpStream>) -> Result<Request, String> {
        let mut headers: HashMap<String, String> = HashMap::new();
        let mut m = String::new();
        buf.read_line(&mut m).await.map_err(|_| "Failed a read")?;
        let (method, protocol) = Self::method_and_procol_from_line(m)?;
        loop {
            let mut st = String::new();
            buf.read_line(&mut st).await.map_err(|_| "Failed a read")?;
            if st == "\r\n" {
                break;
            }
            if let Some((k, v)) = st.split_once(':') {
                headers.insert(k.trim().to_string(), v.trim().to_string());
            }
        }
        let key = match headers.get("Key") {
            Some(k) => digest(k.to_owned()),
            None => digest(""),
        };
        let permissions = Self::get_permissions(&key);

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
                    _headers: headers,
                    body: Some(body),
                    permissions,
                })
            }
            None => Ok(Request {
                method,
                protocol,
                _headers: headers,
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
        let a = AddressListener::new(
            addrs,
            Arc::new(AtomicBool::new(false)),
            self.ps.clone(),
            self.state.clone(),
        )
        .await?;
        self.address_listeners.push(a);
        Ok(())
    }
    pub fn new(ps: Sender<PlayerMessage>, state: Arc<Mutex<PlayerState>>) -> RemoteHandler {
        RemoteHandler {
            ps,
            state,
            address_listeners: vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        sync::{atomic::AtomicBool, Arc, Mutex},
        time::Duration,
    };

    use serial_test::serial;
    use tokio::sync::mpsc::channel;

    use crate::player_state::PlayerState;

    use super::AddressListener;

    fn mock_status() -> Arc<Mutex<PlayerState>> {
        Arc::new(Mutex::new(PlayerState {
            now_playing: None,
            queue: VecDeque::new(),
            volume: 1.0,
            speed: 1.0,
            paused: true,
            source_duration: None,
        }))
    }

    async fn create_valid_listener() -> AddressListener {
        let (s, _) = channel(32);
        let a = AddressListener::new(
            "127.0.0.1:8000".to_string(),
            Arc::new(AtomicBool::new(false)),
            s,
            mock_status(),
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
            mock_status(),
        )
        .await;
        assert!(adrl.is_err());
        let adrl = AddressListener::new(
            "195.251.52.14:90".to_string(),
            Arc::new(AtomicBool::new(false)),
            s,
            mock_status(),
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
    async fn test_stopping() {
        let a = create_valid_listener().await;
        a.stop();
        let ip = format!("http://{}", a.address);
        let resp = reqwest::get(ip).await;
        assert!(resp.is_err())
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
        create_valid_listener().await;
        tokio::time::sleep(Duration::from_secs(100)).await;
    }
    #[tokio::test]
    #[ignore = "Manual test"]
    #[serial]
    async fn test_download_list() {
        create_valid_listener().await;
        todo!()
    }
}

use std::{
    collections::HashMap,
    sync::{atomic::AtomicBool, mpsc::Sender, Arc, Mutex},
    thread::{self},
    time::Duration,
};

use base64::{engine, Engine};


use sha256::digest;
use tokio::{
    io::{AsyncWriteExt, BufReader, AsyncBufReadExt},
    net::{TcpListener, TcpStream},
    select,
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
    method : String,
    protocol: String,
    headers: HashMap<String, String>,
    body: Option<String>,
}

impl AddressListener {
    const SUCCESS: &str = "HTTP/1.1 200 Ok \r\n\r\n";
    const FORBIDDEN: &str = "HTTP/1.1 401 Unauthorized \r\n\r\n";
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
        tokio::spawn(async move {
            let handle = task::spawn(async move {
                println!("Connection Accepted:");
                loop {
                    let (s, _a) = lister.accept().await.unwrap();
                    println!("Connection Accepted:");
                    Self::handle_request(s).await;
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

    async fn handle_request(mut s: tokio::net::TcpStream) {
        let mut buf = BufReader::new(&mut s);
        let mut body: String = String::new();
        let request = Self::parse_request(buf).await;

        // Get

        
        s.write_all(SUCCESS.as_bytes()).await.unwrap();
    }

    async fn parse_request(mut buf: BufReader<&mut TcpStream>) -> Result<Request, &str>{
        let mut headers: HashMap<String, String> = HashMap::new();
        let mut method = String::new();
        buf.read_line(&mut method);

        loop {
            let mut st = String::new();
            buf.read_line(&mut st);
            if st != "\r\n"{
                break;
            }
            if let Some((k,v)) = st.split_once(":"){
                headers.insert(k.to_string(), v.to_string());
            }
        }
        return match headers.get("Content-Length") {
            Some(_) => todo!(),
            None => Err("Unable to get content length for request that requires "),
        }
    }

    fn method_and_procol_from_line(line: String) -> Result<(String, String), String>{
        let (f,s,t) = line.split(" ").collect::<Vec<&str>>().get(0..2).map;
    }

    fn stop(&self) {
        self.stop_handle.store(true, SeqCst);
    }
}

impl RemoteHandler{
    pub fn list_listeners(&self) -> Vec<&str> {
        self.address_listeners.iter().map(|a| a.address.as_str()).collect()
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

    pub async fn new_listener(&mut self, addrs: String) -> Result<(), String>{
        let a = AddressListener::new(addrs, Arc::new(AtomicBool::new(false)), self.ps.clone()).await?;
        self.address_listeners.push(a);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{atomic::AtomicBool, mpsc, Arc},
    };


    use serial_test::serial;

    use crate::commands::PlayerMessage;

    use super::AddressListener;

    async fn create_valid_listener() -> AddressListener {
        let (s, _) = mpsc::channel::<PlayerMessage>();
        let a = AddressListener::new("127.0.0.1:8000".to_string(), Arc::new(AtomicBool::new(false)), s).await;
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
        let (s, _) = mpsc::channel::<PlayerMessage>();
        let adrl = AddressListener::new(
            "slakhfjaskghak".to_string(),
            Arc::new(AtomicBool::new(false)),
            s.clone(),
        ).await;
        assert!(adrl.is_err());
        let adrl = AddressListener::new(
            "195.251.52.14:90".to_string(),
            Arc::new(AtomicBool::new(false)),
            s,
        ).await;
        assert!(adrl.is_err())
    }

    #[tokio::test]
    #[serial]
    async fn test_response() {
        let adrl = create_valid_listener().await;
        let ip = format!("http://{}",adrl.address);
        let resp = reqwest::get(ip).await;
        assert!(resp.is_ok());
        resp.unwrap();
    }

    #[tokio::test]
    #[serial]
    #[should_panic]
    async fn test_stopping(){
        let a = create_valid_listener().await;
        a.stop();
        test_response();
    }
}

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
    println!("Returned");
}

fn handle_stream(mut stream: TcpStream, ps: Sender<PlayerMessage>, state: Arc<Mutex<PlayerState>>) {
    let mut reader = BufReader::new(&mut stream);

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

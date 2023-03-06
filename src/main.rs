pub mod commands;
pub mod song;
pub mod player_state;

use std::collections::VecDeque;
use std::ffi::OsString;
use std::io::{BufReader, stdin, BufRead};
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{Sender, Receiver};
use std::sync::{mpsc, Arc};
use std::process::exit;
use std::*;
use std::time::Duration;
use commands::PlayerMessage;
use rodio::{OutputStream, Sink, Source};
use std::net::TcpStream;
use std::io::prelude::*;
use std::collections::HashMap;
use sync::atomic::Ordering::*;

use crate::player_state::PlayerState;
use crate::song::Song;

static SUCCESS : &str = "HTTP/1.1 200 Ok \r\n\r\n";

fn main() {
    println!("Starting player");
    let (ps, pr) = mpsc::channel::<commands::PlayerMessage>();
    let (state_sender, state_reciever) = mpsc::channel::<PlayerState>();
    let stop_remote : Arc<AtomicBool> = Arc::new(AtomicBool::new(false));

    
    std::thread::spawn(move || {
        let mut queue : VecDeque<Song> = VecDeque::new();
        let mut now_playing : Option<Song> = None;
        let mut current_duration: Option<Duration> = None;
        let (_stream, stream_handle) = OutputStream::try_default().unwrap();
        let sink = Sink::try_new(&stream_handle).unwrap();

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
                        },
                        Err(e) => println!("Error reached when appending: {:#?}", e)
                    }
                }
            }

            // Handle a message if one is recieved
            let message_or_error = pr.try_recv();
            match message_or_error {
                Ok(message) => {
                    match message {
                        PlayerMessage::Stop => {
                            sink.stop();
                        },
                        PlayerMessage::Pause => sink.pause(),
                        PlayerMessage::Play => sink.play(),
                        PlayerMessage::Volume(v) => sink.set_volume(v),
                        PlayerMessage::Skip(n) => {
                            sink.stop();
                            println!("Queue has: {:?} items", queue.len());
                            for _ in 1..n {
                                queue.pop_front();
                                println!("Popped")
                            }
                            println!("Now queue has: {:?} items", queue.len());
                        },
                        PlayerMessage::Add(s) => {
                            let song = Song{
                                name: s.clone(),
                                artist: "Artist Unknown".to_string(),
                                url: "Url Unknown".to_string(),
                                path: OsString::from(s),
                            };
                            queue.push_back(song);
                        }
                        PlayerMessage::Clear => sink.clear(),
                        PlayerMessage::Speed(s) => sink.set_speed(s),
                        PlayerMessage::Status => state_sender.send(PlayerState{
                            now_playing : now_playing.clone(),
                            queue : queue.clone(),
                            volume: sink.volume(),
                            speed: sink.speed(),
                            paused: sink.is_paused(),
                            source_duration: current_duration,
                            
                        }).unwrap(),
                }
            },
                Err(_) => (),
            }
        }
});

    for command in stdin().lock().lines(){
        match command {
            Ok(command) => handle_command(command.trim(), ps.clone(), & state_reciever, stop_remote.clone()),
            Err(_) => println!("Error handling input stream")
         }
    }
    exit_program()
}

fn handle_command(command : & str, ps : Sender<PlayerMessage>,sr : & Receiver<PlayerState>, stop_remote : Arc<AtomicBool>){
    let (command, value) = command.split_once(" ").unwrap_or((command, ""));
        match command {
            "list" => println!("{:?}", list_songs()),
            "volume" => ps.send(PlayerMessage::Volume(value.parse::<f32>().unwrap_or(1.0))).unwrap(),
            "add" => ps.send(PlayerMessage::Add("songs/".to_owned()+&value.to_string())).unwrap(),
            "play" | "continue" => ps.send(PlayerMessage::Play).unwrap(),
            "stop" => ps.send(PlayerMessage::Stop).unwrap(),
            "skip" => ps.send(PlayerMessage::Skip(value.parse::<usize>().unwrap_or(1))).unwrap(),
            "clear" => ps.send(PlayerMessage::Clear).unwrap(),
            "pause" => ps.send(PlayerMessage::Pause).unwrap(),
            "exit" => exit_program(),
            "speed" => ps.send(PlayerMessage::Speed((value.parse::<f32>()).unwrap_or(1.0))).unwrap(),
            "remote" => {
                match value {
                    "start" => {
                        start_remote(ps.clone(), stop_remote.clone());
                        stop_remote.store(false, SeqCst)
                    },
                    "stop" => stop_remote.store(true, SeqCst),
                    _ => println!("Unknown subcommand of'remote'")
                }
            }
            "now" | "nowplaying" | "current" | "np"=> {
                ps.send(PlayerMessage::Status).unwrap();
                let status = sr.recv();
                match status {
                    Ok(status) => println!("{:?}", status.now_playing),
                    Err(e) => println!("Unable to get status for {:#?}", e)
                }
            },
            _ => println!("Unknown command"),
        }
}

fn list_songs() -> Vec<String> {
    let mut song_list : Vec<String> = Vec::new();
    let dir = fs::read_dir("songs/").unwrap();
    for file in dir {
        song_list.push(file.unwrap().file_name().to_str().unwrap().to_string());
    }
    return song_list;
}

fn start_remote(ps : Sender<PlayerMessage>, stop_remote : Arc<AtomicBool>){
    let a = thread::spawn(move||{
        let addr = "192.168.2.116:8008";
        let listener = std::net::TcpListener::bind(addr).unwrap();
        println!("Remote started on: {}", addr);
        listener.set_nonblocking(true);
        for stream in listener.incoming() {
            if stop_remote.load(SeqCst) {
                break;
            }
            match stream {
                Ok(stream) => {
                    println!("Connection established");
                    handle_stream(stream, ps.clone());
                },
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => (),
                Err(e) => panic!("Encountered an error {e}")
            }
        }
        println!("Remote ended")
    });
}

fn handle_stream(mut stream : TcpStream ,ps : Sender<PlayerMessage>){
    let mut reader = BufReader::new(&mut stream);
    
    let mut header_map: HashMap<String, String> = HashMap::new();
    let mut request = String::new();
    reader.read_line(&mut request).unwrap();
    loop {
        let mut buffer : String = String::new();
        let result = reader.read_line(&mut buffer);
        match result {
            Ok(0) => break,
            Ok(_) => {
                match buffer.split_once(" ").unwrap_or(("","")) {
                    ("", "") => break,
                    (k,v) => {
                        header_map.insert(k.trim().to_owned(), v.trim().to_owned());
                        ()
                    },
                }

            },
            Err(e) => {
                println!("Error {:?} reached", e);
                break;
            }
        }
    }
    let mut body = String::new();

    match header_map.get("Content-Length:") {
        Some(v) => {
            let mut buffer = vec![0u8; v.parse::<usize>().unwrap_or(0)];
            reader.read_exact(&mut buffer).unwrap();
            body = String::from_utf8(buffer).unwrap_or("".to_string());
        },
        None => (),
    }

    //println!("Header map: {:?}", header_map);
    //println!("Body is: {:?}", body);
    //println!("Request is : {:?}", request);

    let response = match request.trim(){
        "GET / HTTP/1.1" => SUCCESS.to_string(),
        "POST /pause HTTP/1.1" => {
            ps.send(PlayerMessage::Pause).unwrap();
            SUCCESS.to_string()
        },
        "POST /play HTTP/1.1" =>{
            ps.send(PlayerMessage::Play).unwrap();
            SUCCESS.to_string()
        }
        "POST /skip HTTP/1.1" => {
            ps.send(PlayerMessage::Skip(body.parse::<usize>().unwrap_or(1))).unwrap();
            SUCCESS.to_string()
        }
        "POST /add HTTP/1.1" => {
            ps.send(PlayerMessage::Add(body)).unwrap();
            SUCCESS.to_string()
        },
        "POST /volume HTTP/1.1" =>{
            ps.send(PlayerMessage::Volume(body.parse::<f32>().unwrap_or(1.0))).unwrap();
            SUCCESS.to_string()
        }
        "POST /speed HTTP/1.1" =>{
            ps.send(PlayerMessage::Speed(body.parse::<f32>().unwrap_or(1.0))).unwrap();
            SUCCESS.to_string()
        } 
        "GET /list HTTP/1.1" => {
            let list = list_songs();
            let json = serde_json::to_string(&list).unwrap(); 
            json_to_https(json)
        }
        _ => "HTTP/1.1 404 NOT FOUND\r\n\r\n".to_string(),
    };

    println!("Responce is: {:?}", response);
    stream.write_all(response.as_bytes()).unwrap();

}

fn json_to_https(json : String) -> String{
    let len = json.as_bytes().len();
    return format!("HTTP/1.1 200 Ok\r\nContent-Type: application/json\r\nContent-Length: {len}\r\n\r\n{json}");
}

fn exit_program(){
    println!("Exitting");
    exit(0)
}

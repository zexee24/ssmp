pub mod commands;

use std::collections::VecDeque;
use std::fs::File;
use std::io::{BufReader, stdin, BufRead};
use std::str::from_utf8;
use std::sync::mpsc::Sender;
use std::sync::mpsc;
use std::process::exit;
use std::*;
use commands::PlayerMessage;
use rodio::{OutputStream, Sink, Decoder};
use std::net::TcpStream;
use std::io::prelude::*;
use std::collections::HashMap;

static SUCCESS : &str = "HTTP/1.1 200 Ok \r\n\r\n";

fn main() {
    println!("Starting player");
    let (ps, pr) = mpsc::channel::<commands::PlayerMessage>();
    
    std::thread::spawn(move || {
        let mut queue = VecDeque::new();
        let (_stream, stream_handle) = OutputStream::try_default().unwrap();
        let sink = Sink::try_new(&stream_handle).unwrap();

        loop {
            if sink.empty() && queue.len() > 0 {
                sink.append(queue.pop_front().unwrap())
            }
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
                            let file_to_open = File::open(s.clone());
                            if !file_to_open.is_ok() {
                                println!("File {:?} not found", s);
                            } else {
                                let file = BufReader::new(file_to_open.unwrap());
                                let source = Decoder::new(file).unwrap();
                                queue.push_back(source);
                            }
                        }
                        PlayerMessage::Clear => sink.clear(),
                        PlayerMessage::Speed(s) => sink.set_speed(s),
                }
            },
                Err(_) => (),
            }
        }
});

    println!("Enter command:");
    for command in stdin().lock().lines(){
        match command {
            Ok(command) => handle_command(command.trim(), ps.clone()),
            Err(_) => println!("Error handling input stream")
         }
    println!("Enter command:");
    }
    exit_program()
}

fn handle_command(command : & str, ps : Sender<PlayerMessage>){
    let (command, value) = command.split_once(" ").unwrap_or((command, ""));
        match command {
            "list" => {
                let dir = fs::read_dir("songs/").unwrap();
                for file in dir {
                    println!("{:?}", file.unwrap().file_name())
                }
            },
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
                    "start" => start_remote(ps.clone()),
                    "stop" => (), /* Work in progress*/
                    _ => println!("Unknown subcommand of'remote'")
                }
            }
            _ => println!("Unknown command"),
        }
}

fn start_remote(ps : Sender<PlayerMessage>){
    let _ = thread::spawn(move||{
        let addr = "192.168.2.116:8008";
        let listener = std::net::TcpListener::bind(addr).unwrap();
        println!("Remote started on: {}", addr);

        for stream in listener.incoming(){
            println!("Connection established");
            handle_stream(stream.unwrap(), ps.clone());
        }
    });
}

fn handle_stream(mut stream : TcpStream ,ps : Sender<PlayerMessage>){
    let mut reader = BufReader::new(&mut stream);
    
    let mut header_map: HashMap<String, String> = HashMap::new();
    let mut request = String::new();
    reader.read_line(&mut request);
    loop {
        let mut buffer : String = String::new();
        let result = reader.read_line(&mut buffer);
        match result {
            Ok(0) => break,
            Ok(n) => {
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
        "GET / HTTP/1.1" => SUCCESS,
        "POST /pause HTTP/1.1" => {
            ps.send(PlayerMessage::Pause).unwrap();
            SUCCESS
        },
        "POST /play HTTP/1.1" =>{
            ps.send(PlayerMessage::Play).unwrap();
            SUCCESS
        }
        "POST /skip HTTP/1.1" => {
            ps.send(PlayerMessage::Skip(body.parse::<usize>().unwrap_or(1))).unwrap();
            SUCCESS
        }
        "POST /add HTTP/1.1" => {
            ps.send(PlayerMessage::Add(body)).unwrap();
            SUCCESS
        },
        "GET /list HTTP/1.1" => {
            SUCCESS
        }
        _ => "HTTP/1.1 404 NOT FOUND\r\n\r\n",
    };

    println!("Responce is: {:?}", response);
    stream.write_all(response.as_bytes()).unwrap();

}

fn exit_program(){
    println!("Exitting");
    exit(0)
}

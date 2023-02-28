pub mod commands;

use std::collections::VecDeque;
use std::fs::File;
use std::io::{BufReader, stdin, BufRead};
use std::sync::mpsc::Sender;
use std::sync::mpsc;
use std::process::exit;
use std::*;
use commands::PlayerMessage;
use rodio::{OutputStream, Sink, Decoder};

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
                            for _ in 1..n-1 {
                                queue.pop_front();
                            }
                        },
                        PlayerMessage::Add(s) => {
                            let file_to_open = File::open(s);
                            if !file_to_open.is_ok() {
                                println!("File not found");
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
            _ => println!("Unknown command"),
        }
}

fn exit_program(){
    println!("Exitting");
    exit(0)
}
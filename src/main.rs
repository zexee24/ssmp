pub mod commands;

use std::fs::File;
use std::io::{BufReader, stdin, BufRead};
use std::sync::mpsc::Sender;
use std::{sync::mpsc};
use std::process::exit;
use std::*;
use commands::PlayerMessage;
use rodio::{OutputStream, Sink, Decoder, queue::queue};

fn main() {
    println!("Starting player");
    let (ps, pr) = mpsc::channel::<commands::PlayerMessage>();
    
    std::thread::spawn(move || {
        let (que_input, que_output) = queue::<f32>(true);
        let (_stream, stream_handle) = OutputStream::try_default().unwrap();
        let sink = Sink::try_new(&stream_handle).unwrap();
        loop {
            let message = pr.recv().unwrap();
            match message {
                PlayerMessage::Stop => {
                    sink.stop();
                },
                PlayerMessage::Pause => sink.pause(),
                PlayerMessage::Play => sink.play(),
                PlayerMessage::Volume(v) => sink.set_volume(v),
                PlayerMessage::Skip(n) => sink.skip_one(),
                PlayerMessage::Add(s) => {
                    let file_to_open = File::open(s);
                    if !file_to_open.is_ok() {
                        println!("File not found");
                    } else {
                        let file = BufReader::new(file_to_open.unwrap());
                        let source = Decoder::new(file).unwrap();
                        sink.append(source);
                    }
                }
                PlayerMessage::Clear => sink.clear(),
                PlayerMessage::Speed(s) => sink.set_speed(s),
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
            "skip" => ps.send(PlayerMessage::Skip(value.parse::<u64>().unwrap_or(1))).unwrap(),
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
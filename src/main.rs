use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

type Store = Arc<Mutex<HashMap<String, String>>>;

fn main() {
    let store: Store = Arc::new(Mutex::new(HashMap::new()));

    let listener = TcpListener::bind("127.0.0.1:6379").expect("Failed to bind to port 6379");
    println!("🦀 RedRust listening on 127.0.0.1:6379");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let store = Arc::clone(&store);
                std::thread::spawn(|| {
                    handle_client(stream, store);
                });
            }
            Err(e) => {
                eprintln!("Error accepting connection: {}", e);
            }
        }
    }
}

fn handle_client(mut stream: TcpStream, store: Store) {
    let peer = stream.peer_addr().unwrap();
    println!("Client connected: {}", peer);

    let reader = BufReader::new(stream.try_clone().unwrap());

    for line in reader.lines() {
        match line {
            Ok(command) => {
                let response = process_command(&command, &store);
                if let Err(e) = stream.write_all(response.as_bytes()) {
                    eprintln!("Error writing to client: {}", e);
                    break;
                }
            }
            Err(_) => break,
        }
    }

    println!("Client disconnected: {}", peer);
}

fn process_command(command: &str, store: &Store) -> String {
    let parts: Vec<&str> = command.trim().split_whitespace().collect();

    if parts.is_empty() {
        return "-ERR empty command\r\n".to_string();
    }

    let cmd = parts[0].to_uppercase();
    let mut db = store.lock().unwrap();

    match cmd.as_str() {
        "SET" => {
            if parts.len() != 3 {
                return "-ERR usage: SET key value\r\n".to_string();
            }
            db.insert(parts[1].to_string(), parts[2].to_string());
            "+OK\r\n".to_string()
        }
        "GET" => {
            if parts.len() != 2 {
                return "-ERR usage: GET key\r\n".to_string();
            }
            match db.get(parts[1]) {
                Some(value) => format!("${}\r\n{}\r\n", value.len(), value),
                None => "$-1\r\n".to_string(),
            }
        }
        "DEL" => {
            if parts.len() != 2 {
                return "-ERR usage: DEL key\r\n".to_string();
            }
            let removed = db.remove(parts[1]).is_some();
            format!(":{}\r\n", if removed { 1 } else { 0 })
        }
        "KEYS" => {
            let keys: Vec<&String> = db.keys().collect();
            let mut response = format!("*{}\r\n", keys.len());
            for key in keys {
                response.push_str(&format!("${}\r\n{}\r\n", key.len(), key));
            }
            response
        }
        "PING" => "+PONG\r\n".to_string(),
        _ => "-ERR unknown command\r\n".to_string(),
    }
}
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::{Instant, Duration};

// Store both the value and optional expiration time
#[derive(Clone)]
struct Entry {
    value: String,
    expires_at: Option<Instant>,
}

type Store = Arc<Mutex<HashMap<String, Entry>>>;

fn main() {
    let store: Store = Arc::new(Mutex::new(HashMap::new()));

    // Spawn cleanup thread for expired keys
    let cleanup_store = Arc::clone(&store);
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_secs(1));
            cleanup_expired(&cleanup_store);
        }
    });

    let listener = TcpListener::bind("127.0.0.1:6379").expect("Failed to bind");
    println!("🦀 RedRust listening on 127.0.0.1:6379");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let store = Arc::clone(&store);
                std::thread::spawn(|| handle_client(stream, store));
            }
            Err(e) => eprintln!("Error: {}", e),
        }
    }
}

fn cleanup_expired(store: &Store) {
    let mut db = store.lock().unwrap();
    let now = Instant::now();
    let expired: Vec<String> = db
        .iter()
        .filter(|(_, entry)| {
            entry.expires_at.map(|exp| exp <= now).unwrap_or(false)
        })
        .map(|(key, _)| key.clone())
        .collect();

    for key in expired {
        db.remove(&key);
    }
}

fn is_expired(entry: &Entry) -> bool {
    entry.expires_at.map(|exp| exp <= Instant::now()).unwrap_or(false)
}

fn handle_client(mut stream: TcpStream, store: Store) {
    let peer = stream.peer_addr().unwrap();
    println!("Client connected: {}", peer);

    let reader = BufReader::new(stream.try_clone().unwrap());

    for line in reader.lines() {
        match line {
            Ok(command) => {
                let response = process_command(&command, &store);
                if stream.write_all(response.as_bytes()).is_err() {
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
            if parts.len() < 3 {
                return "-ERR usage: SET key value [EX seconds]\r\n".to_string();
            }
            let key = parts[1].to_string();
            let value = parts[2].to_string();
            let expires_at = if parts.len() >= 5 && parts[3].to_uppercase() == "EX" {
                match parts[4].parse::<u64>() {
                    Ok(secs) => Some(Instant::now() + Duration::from_secs(secs)),
                    Err(_) => return "-ERR invalid expire time\r\n".to_string(),
                }
            } else {
                None
            };

            db.insert(key, Entry { value, expires_at });
            "+OK\r\n".to_string()
        }

        "GET" => {
            if parts.len() != 2 {
                return "-ERR usage: GET key\r\n".to_string();
            }
            match db.get(parts[1]) {
                Some(entry) if !is_expired(entry) => {
                    format!("${}\r\n{}\r\n", entry.value.len(), entry.value)
                }
                _ => "$-1\r\n".to_string(),
            }
        }

        "EXPIRE" => {
            if parts.len() != 3 {
                return "-ERR usage: EXPIRE key seconds\r\n".to_string();
            }
            let seconds = match parts[2].parse::<u64>() {
                Ok(s) => s,
                Err(_) => return ":0\r\n".to_string(),
            };

            match db.get_mut(parts[1]) {
                Some(entry) => {
                    entry.expires_at = Some(Instant::now() + Duration::from_secs(seconds));
                    ":1\r\n".to_string()
                }
                None => ":0\r\n".to_string(),
            }
        }

        "TTL" => {
            if parts.len() != 2 {
                return "-ERR usage: TTL key\r\n".to_string();
            }
            match db.get(parts[1]) {
                Some(entry) => match entry.expires_at {
                    Some(exp) => {
                        let remaining = exp.duration_since(Instant::now()).as_secs();
                        format!(":{}\r\n", remaining)
                    }
                    None => ":-1\r\n".to_string(), // No expiration
                },
                None => ":-2\r\n".to_string(), // Key doesn't exist
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
            let now = Instant::now();
            let keys: Vec<&String> = db
                .iter()
                .filter(|(_, entry)| entry.expires_at.map(|exp| exp > now).unwrap_or(true))
                .map(|(key, _)| key)
                .collect();

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
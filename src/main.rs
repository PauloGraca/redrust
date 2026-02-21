use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::{Instant, Duration};

// Now we support multiple data types!
#[derive(Clone)]
enum Value {
    String(String),
    List(Vec<String>),
}

struct Entry {
    value: Value,
    expires_at: Option<Instant>,
}

type Store = Arc<Mutex<HashMap<String, Entry>>>;

fn main() {
    let store: Store = Arc::new(Mutex::new(HashMap::new()));

    // Cleanup thread for expired keys
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
        // ========== STRING COMMANDS ==========
        "SET" => {
            if parts.len() < 3 {
                return "-ERR usage: SET key value [EX seconds]\r\n".to_string();
            }
            let key = parts[1].to_string();
            let value = Value::String(parts[2].to_string());
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
                    match &entry.value {
                        Value::String(s) => format!("${}\r\n{}\r\n", s.len(), s),
                        Value::List(_) => "-ERR Operation against a key holding the wrong kind of value\r\n".to_string(),
                    }
                }
                _ => "$-1\r\n".to_string(),
            }
        }

        // ========== LIST COMMANDS ==========
        "LPUSH" => {
            if parts.len() < 3 {
                return "-ERR usage: LPUSH key value [value ...]\r\n".to_string();
            }
            let key = parts[1];

            let entry = db.entry(key.to_string()).or_insert_with(|| Entry {
                value: Value::List(Vec::new()),
                expires_at: None,
            });

            match &mut entry.value {
                Value::List(list) => {
                    // Insert all values at the front (in reverse order of args)
                    for value in parts[2..].iter().rev() {
                        list.insert(0, value.to_string());
                    }
                    format!(":{}\r\n", list.len())
                }
                _ => "-ERR Operation against a key holding the wrong kind of value\r\n".to_string(),
            }
        }

        "RPUSH" => {
            if parts.len() < 3 {
                return "-ERR usage: RPUSH key value [value ...]\r\n".to_string();
            }
            let key = parts[1];

            let entry = db.entry(key.to_string()).or_insert_with(|| Entry {
                value: Value::List(Vec::new()),
                expires_at: None,
            });

            match &mut entry.value {
                Value::List(list) => {
                    for value in &parts[2..] {
                        list.push(value.to_string());
                    }
                    format!(":{}\r\n", list.len())
                }
                _ => "-ERR Operation against a key holding the wrong kind of value\r\n".to_string(),
            }
        }

        "LPOP" => {
            if parts.len() != 2 {
                return "-ERR usage: LPOP key\r\n".to_string();
            }
            match db.get_mut(parts[1]) {
                Some(ref mut entry) if !is_expired(entry) => {
                    match &mut entry.value {
                        Value::List(list) => {
                            if let Some(val) = list.remove(0) {
                                let response = format!("${}\r\n{}\r\n", val.len(), val);
                                if list.is_empty() {
                                    db.remove(parts[1]);
                                }
                                response
                            } else {
                                "$-1\r\n".to_string()
                            }
                        }
                        _ => "-ERR Operation against a key holding the wrong kind of value\r\n".to_string(),
                    }
                }
                _ => "$-1\r\n".to_string(),
            }
        }

        "RPOP" => {
            if parts.len() != 2 {
                return "-ERR usage: RPOP key\r\n".to_string();
            }
            match db.get_mut(parts[1]) {
                Some(ref mut entry) if !is_expired(entry) => {
                    match &mut entry.value {
                        Value::List(list) => {
                            if let Some(val) = list.pop() {
                                let response = format!("${}\r\n{}\r\n", val.len(), val);
                                if list.is_empty() {
                                    db.remove(parts[1]);
                                }
                                response
                            } else {
                                "$-1\r\n".to_string()
                            }
                        }
                        _ => "-ERR Operation against a key holding the wrong kind of value\r\n".to_string(),
                    }
                }
                _ => "$-1\r\n".to_string(),
            }
        }

        "LLEN" => {
            if parts.len() != 2 {
                return "-ERR usage: LLEN key\r\n".to_string();
            }
            match db.get(parts[1]) {
                Some(entry) if !is_expired(entry) => {
                    match &entry.value {
                        Value::List(list) => format!(":{}\r\n", list.len()),
                        _ => "-ERR Operation against a key holding the wrong kind of value\r\n".to_string(),
                    }
                }
                _ => ":0\r\n".to_string(),
            }
        }

        "LRANGE" => {
            if parts.len() != 4 {
                return "-ERR usage: LRANGE key start stop\r\n".to_string();
            }
            let start: i64 = parts[2].parse().unwrap_or(0);
            let stop: i64 = parts[3].parse().unwrap_or(-1);

            match db.get(parts[1]) {
                Some(entry) if !is_expired(entry) => {
                    match &entry.value {
                        Value::List(list) => {
                            let len = list.len() as i64;
                            let actual_start = if start < 0 { len + start } else { start }.max(0) as usize;
                            let actual_stop = if stop < 0 { len + stop } else { stop }.min(len - 1) as usize;

                            let mut response = format!("*{}\r\n", if actual_start <= actual_stop { actual_stop - actual_start + 1 } else { 0 });

                            for i in actual_start..=actual_stop.min(list.len().saturating_sub(1)) {
                                if i < list.len() {
                                    let val = &list[i];
                                    response.push_str(&format!("${}\r\n{}\r\n", val.len(), val));
                                }
                            }
                            response
                        }
                        _ => "-ERR Operation against a key holding the wrong kind of value\r\n".to_string(),
                    }
                }
                _ => "*0\r\n".to_string(),
            }
        }

        // ========== OTHER COMMANDS ==========
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
                    None => ":-1\r\n".to_string(),
                },
                None => ":-2\r\n".to_string(),
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

        "TYPE" => {
            if parts.len() != 2 {
                return "-ERR usage: TYPE key\r\n".to_string();
            }
            match db.get(parts[1]) {
                Some(entry) if !is_expired(entry) => {
                    let type_str = match &entry.value {
                        Value::String(_) => "string",
                        Value::List(_) => "list",
                    };
                    format!("+{}\r\n", type_str)
                }
                _ => "+none\r\n".to_string(),
            }
        }

        "PING" => "+PONG\r\n".to_string(),

        _ => "-ERR unknown command\r\n".to_string(),
    }
}
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::{Instant, Duration, SystemTime, UNIX_EPOCH};
use serde::{Serialize, Deserialize};

// Serializable entry for persistence
#[derive(Clone, Serialize, Deserialize)]
struct SerializableEntry {
    value: SerializableValue,
    #[serde(with = "option_duration")]
    expires_in_secs: Option<u64>, // Store relative time instead of Instant
}

#[derive(Clone, Serialize, Deserialize)]
enum SerializableValue {
    String(String),
    List(Vec<String>),
}

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

// Custom serialization for Option<Duration>
mod option_duration {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(v: &Option<u64>, s: S) -> Result<S::Ok, S::Error> {
        v.serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<u64>, D::Error> {
        Option::<u64>::deserialize(d)
    }
}

fn main() {
    let store: Store = Arc::new(Mutex::new(HashMap::new()));
    
    // Try to load existing data
    load_data(&store, "redrust.rdb");
    
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
    println!("   Commands: SET, GET, DEL, KEYS, EXPIRE, TTL, TYPE, PING");
    println!("   Lists: LPUSH, RPUSH, LPOP, RPOP, LLEN, LRANGE");
    println!("   Persistence: SAVE, BGSAVE, LASTSAVE");
    
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

fn save_data(store: &Store, filename: &str) -> Result<(), String> {
    let db = store.lock().unwrap();
    let now = Instant::now();
    let now_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    
    let serializable: HashMap<String, SerializableEntry> = db
        .iter()
        .filter(|(_, entry)| !is_expired(entry))
        .map(|(key, entry)| {
            let value = match &entry.value {
                Value::String(s) => SerializableValue::String(s.clone()),
                Value::List(l) => SerializableValue::List(l.clone()),
            };
            
            let expires_in_secs = entry.expires_at.map(|exp| {
                let remaining = exp.duration_since(now).as_secs();
                now_secs + remaining
            });
            
            (key.clone(), SerializableEntry { value, expires_in_secs })
        })
        .collect();
    
    let json = serde_json::to_string_pretty(&serializable)
        .map_err(|e| format!("Serialization error: {}", e))?;
    
    std::fs::write(filename, json)
        .map_err(|e| format!("Write error: {}", e))?;
    
    Ok(())
}

fn load_data(store: &Store, filename: &str) {
    let json = match std::fs::read_to_string(filename) {
        Ok(content) => content,
        Err(_) => {
            println!("No existing database found, starting fresh");
            return;
        }
    };
    
    let serializable: HashMap<String, SerializableEntry> = match serde_json::from_str(&json) {
        Ok(data) => data,
        Err(e) => {
            eprintln!("Failed to load database: {}", e);
            return;
        }
    };
    
    let now_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let now = Instant::now();
    
    let mut db = store.lock().unwrap();
    for (key, entry) in serializable {
        // Skip expired entries
        if let Some(exp) = entry.expires_in_secs {
            if exp <= now_secs {
                continue;
            }
        }
        
        let value = match entry.value {
            SerializableValue::String(s) => Value::String(s),
            SerializableValue::List(l) => Value::List(l),
        };
        
        let expires_at = entry.expires_in_secs.map(|exp| {
            let remaining = exp.saturating_sub(now_secs);
            now + Duration::from_secs(remaining)
        });
        
        db.insert(key, Entry { value, expires_at });
    }
    
    println!("Loaded {} keys from {}", db.len(), filename);
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
                            if list.is_empty() {
                                "$-1\r\n".to_string()
                            } else {
                                let val = list.remove(0);
                                let response = format!("${}\r\n{}\r\n", val.len(), val);
                                if list.is_empty() {
                                    db.remove(parts[1]);
                                }
                                response
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
        
        // ========== PERSISTENCE COMMANDS ==========
        "SAVE" => {
            drop(db); // Release lock before saving
            match save_data(store, "redrust.rdb") {
                Ok(()) => "+OK\r\n".to_string(),
                Err(e) => format!("-ERR {}\r\n", e),
            }
        }
        
        "BGSAVE" => {
            let store_clone = Arc::clone(store);
            std::thread::spawn(move || {
                match save_data(&store_clone, "redrust.rdb") {
                    Ok(()) => println!("Background save completed"),
                    Err(e) => eprintln!("Background save failed: {}", e),
                }
            });
            "+Background saving started\r\n".to_string()
        }
        
        "LASTSAVE" => {
            let timestamp = std::fs::metadata("redrust.rdb")
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(-1);
            format!(":{}\r\n", timestamp)
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

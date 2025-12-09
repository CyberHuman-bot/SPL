use std::fs::File;
use std::io::{Read, Write, BufRead, BufReader};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use std::thread;
use std::collections::HashMap;

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use rand::Rng;

// 2 MB chunks
const CHUNK_SIZE: usize = 2 * 1024 * 1024;
const SERVER_PORT: u16 = 5001;
const DISCOVERY_PORT: u16 = 5000;
const DISCOVERY_TIMEOUT: u64 = 5; // 5 seconds

// ---- Time helper ----
fn current_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

// ---- AES helpers ----
fn encrypt_chunk(key: &[u8], plaintext: &[u8]) -> Vec<u8> {
    let cipher = Aes256Gcm::new_from_slice(key).unwrap();
    let nonce_bytes: [u8; 12] = rand::thread_rng().gen();
    let nonce = Nonce::from_slice(&nonce_bytes);
    let mut ciphertext = cipher.encrypt(nonce, plaintext).unwrap();
    let mut result = nonce_bytes.to_vec();
    result.append(&mut ciphertext);
    result
}

fn decrypt_chunk(key: &[u8], data: &[u8]) -> Result<Vec<u8>, aes_gcm::Error> {
    let cipher = Aes256Gcm::new_from_slice(key).unwrap();
    let nonce = Nonce::from_slice(&data[..12]);
    cipher.decrypt(nonce, &data[12..])
}

// ---- Progress bar ----
fn print_progress(transferred: usize, total: usize, start: std::time::Instant) {
    let percent = if total > 0 {
        (transferred as f64 / total as f64) * 100.0
    } else {
        0.0
    };
    let elapsed = start.elapsed().as_secs_f64().max(0.01);
    let speed = transferred as f64 / (1024.0 * 1024.0) / elapsed;
    let bar_len = 30;
    let filled = ((bar_len as f64) * transferred as f64 / (total as f64)).round() as usize;
    let bar = "█".repeat(filled) + &"-".repeat(bar_len - filled);
    print!("\r[{}] {:.1}% | {:.2} MB/s", bar, percent, speed);
    std::io::stdout().flush().unwrap();
}

// ---- Device Discovery ----
fn discover_devices() -> Vec<String> {
    println!("🔍 Discovering devices on network...");
    
    let socket = UdpSocket::bind("0.0.0.0:0").expect("Failed to bind UDP socket");
    socket.set_read_timeout(Some(Duration::from_secs(DISCOVERY_TIMEOUT))).unwrap();
    
    // Broadcast discovery message
    let discovery_msg = b"SPL_DISCOVER";
    socket.send_to(discovery_msg, "255.255.255.255:5000").ok();
    
    let mut devices = HashMap::new();
    let mut buffer = [0u8; 1024];
    
    while let Ok((len, addr)) = socket.recv_from(&mut buffer) {
        if &buffer[..len] == b"SPL_HERE" {
            let ip = addr.ip().to_string();
            devices.insert(ip.clone(), addr);
            println!("  Found: {}", ip);
        }
    }
    
    let mut device_list: Vec<_> = devices.keys().cloned().collect();
    device_list.sort();
    device_list
}

fn select_device(devices: &[String]) -> Option<String> {
    if devices.is_empty() {
        println!("❌ No devices found");
        return None;
    }
    
    println!("\n📱 Available devices:");
    for (i, ip) in devices.iter().enumerate() {
        println!("  {}: {}", i + 1, ip);
    }
    
    print!("Select device (1-{}): ", devices.len());
    std::io::stdout().flush().unwrap();
    
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).ok()?;
    
    if let Ok(num) = input.trim().parse::<usize>() {
        if num > 0 && (num as usize - 1) < devices.len() {
            return Some(devices[num - 1].clone());
        }
    }
    
    println!("❌ Invalid selection");
    None
}

// ---- Start Receiver Service ----
fn start_receiver_service(outfile: &str) {
    let listener = TcpListener::bind(format!("0.0.0.0:{}", SERVER_PORT)).unwrap();
    println!("📥 Receiver ready on port {}, saving to {}", SERVER_PORT, outfile);
    
    // Also start discovery responder
    thread::spawn(|| discovery_responder());
    
    let (mut stream, addr) = listener.accept().unwrap();
    println!("✅ Connection from {}", addr);
    
    // Receive key first
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).unwrap();
    let key_len = u32::from_be_bytes(len_buf) as usize;
    let mut key = vec![0u8; key_len];
    stream.read_exact(&mut key).unwrap();
    
    let mut file = File::create(outfile).unwrap();
    let start = std::time::Instant::now();
    let mut got_bytes = 0;
    
    loop {
        let mut len_buf = [0u8; 4];
        if stream.read_exact(&mut len_buf).is_err() {
            break;
        }
        let len = u32::from_be_bytes(len_buf) as usize;
        
        let mut encrypted = vec![0u8; len];
        if stream.read_exact(&mut encrypted).is_err() {
            break;
        }
        
        let decrypted = match decrypt_chunk(&key, &encrypted) {
            Ok(data) => data,
            Err(_) => {
                eprintln!("❌ Decryption failed");
                break;
            }
        };
        
        file.write_all(&decrypted).unwrap();
        got_bytes += decrypted.len();
        print_progress(got_bytes, got_bytes, start);
    }
    
    println!("\n✅ File saved as {}", outfile);
}

fn discovery_responder() {
    let socket = UdpSocket::bind(format!("0.0.0.0:{}", DISCOVERY_PORT)).expect("Failed to bind discovery socket");
    let mut buffer = [0u8; 1024];
    
    loop {
        if let Ok((len, _)) = socket.recv_from(&mut buffer) {
            if &buffer[..len] == b"SPL_DISCOVER" {
                socket.send_to(b"SPL_HERE", "255.255.255.255:5000").ok();
            }
        }
    }
}

// ---- TCP Sender ----
fn send_file_tcp(filename: &str, ip: &str) {
    let mut file = File::open(filename).expect("File not found");
    let size = file.metadata().unwrap().len() as usize;
    
    println!(
        "📤 Sending {} ({:.2} MB) → {}:{}",
        filename,
        size as f64 / 1024.0 / 1024.0,
        ip,
        SERVER_PORT
    );
    
    let mut stream = TcpStream::connect(format!("{}:{}", ip, SERVER_PORT)).unwrap();
    
    // Generate a random AES key for this transfer
    let key: [u8; 32] = rand::thread_rng().gen();
    
    // Send key first
    stream.write_all(&(key.len() as u32).to_be_bytes()).unwrap();
    stream.write_all(&key).unwrap();
    
    let start = std::time::Instant::now();
    let mut buffer = vec![0u8; CHUNK_SIZE];
    let mut sent_bytes = 0;
    
    loop {
        let n = file.read(&mut buffer).unwrap();
        if n == 0 {
            break;
        }
        
        let encrypted = encrypt_chunk(&key, &buffer[..n]);
        let len_bytes = (encrypted.len() as u32).to_be_bytes();
        stream.write_all(&len_bytes).unwrap();
        stream.write_all(&encrypted).unwrap();
        
        sent_bytes += n;
        print_progress(sent_bytes, size, start);
    }
    println!("\n✅ Transfer complete");
}

// ---- CLI ----
fn main() {
    let args: Vec<String> = std::env::args().collect();
    
    if args.len() < 2 {
        println!("Usage:");
        println!("  spl send <file>          # Auto-discover & send");
        println!("  spl receive <outfile>    # Receive file");
        println!("  spl -<IP> <file>         # Send to specific IP (legacy)");
        return;
    }
    
    match args[1].as_str() {
        "send" => {
            if args.len() < 3 {
                println!("❌ Missing file argument");
                return;
            }
            
            // Start discovery responder for other devices
            thread::spawn(|| discovery_responder());
            
            // Discover devices
            let devices = discover_devices();
            if let Some(ip) = select_device(&devices) {
                send_file_tcp(&args[2], &ip);
            }
        }
        "receive" => {
            if args.len() < 3 {
                println!("❌ Missing output file argument");
                return;
            }
            start_receiver_service(&args[2]);
        }
        _ if args[1].starts_with("-") => {
            if args.len() < 3 {
                println!("❌ Missing file argument");
                return;
            }
            send_file_tcp(&args[2], &args[1][1..]);
        }
        _ => {
            println!("❌ Invalid command. See usage above.");
        }
    }
}

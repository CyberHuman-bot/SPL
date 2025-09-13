use std::fs::File;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use rand::Rng;
use serde::{Deserialize, Serialize};

const CHUNK_SIZE: usize = 2 * 1024 * 1024; // 2 MB
const CONFIG_FILE: &str = ".spl_config.json";

#[derive(Serialize, Deserialize)]
struct Config {
    key: Vec<u8>,
    port: u16,
}

// ---- Config management ----
fn load_config() -> Config {
    if Path::new(CONFIG_FILE).exists() {
        let f = File::open(CONFIG_FILE).expect("Cannot open config file");
        return serde_json::from_reader(f).expect("Config file corrupted");
    }

    let key: [u8; 32] = rand::thread_rng().gen();
    let port: u16 = 5001;

    let cfg = Config {
        key: key.to_vec(),
        port,
    };

    let f = File::create(CONFIG_FILE).expect("Cannot create config file");
    serde_json::to_writer_pretty(f, &cfg).expect("Cannot write config file");
    println!("🔑 New config generated at {}", CONFIG_FILE);
    cfg
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

// ---- Progress ----
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

// ---- TCP Sender ----
fn send_file_tcp(filename: &str, ip: &str, key: &[u8], port: u16) {
    let mut file = File::open(filename).expect("File not found");
    let size = file.metadata().unwrap().len() as usize;
    println!(
        "📤 Sending {} ({:.2} MB) → {}:{}",
        filename,
        size as f64 / 1024.0 / 1024.0,
        ip,
        port
    );

    let mut stream = TcpStream::connect(format!("{}:{}", ip, port)).expect("Cannot connect to receiver");
    let start = std::time::Instant::now();
    let mut buffer = vec![0u8; CHUNK_SIZE];
    let mut sent_bytes = 0;

    loop {
        let n = file.read(&mut buffer).unwrap_or(0);
        if n == 0 { break; }

        let encrypted = encrypt_chunk(key, &buffer[..n]);
        let len_bytes = (encrypted.len() as u32).to_be_bytes();
        stream.write_all(&len_bytes).unwrap();
        stream.write_all(&encrypted).unwrap();

        sent_bytes += n;
        print_progress(sent_bytes, size, start);
    }
    println!("\n✅ Transfer complete");
}

// ---- TCP Receiver ----
fn receive_file_tcp(outfile: &str, key: &[u8], port: u16) {
    let listener = TcpListener::bind(format!("0.0.0.0:{}", port)).expect("Cannot bind port");
    println!("📥 Receiver ready on port {}, saving to {}", port, outfile);

    let (mut stream, _) = listener.accept().expect("Failed to accept connection");
    let mut file = File::create(outfile).expect("Cannot create output file");
    let start = std::time::Instant::now();
    let mut got_bytes = 0;

    loop {
        let mut len_buf = [0u8; 4];
        if stream.read_exact(&mut len_buf).is_err() { break; }
        let len = u32::from_be_bytes(len_buf) as usize;

        let mut encrypted = vec![0u8; len];
        if stream.read_exact(&mut encrypted).is_err() { break; }

        let decrypted = match decrypt_chunk(key, &encrypted) {
            Ok(data) => data,
            Err(_) => { eprintln!("❌ Decryption failed"); break; }
        };

        file.write_all(&decrypted).unwrap();
        got_bytes += decrypted.len();
        print_progress(got_bytes, got_bytes, start);
    }
    println!("\n✅ File saved as {}", outfile);
}

// ---- CLI ----
fn main() {
    let cfg = load_config();
    let key = &cfg.key;
    let port = cfg.port;

    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        println!("Usage:\n  Sender: spl -<IP> <file>\n  Receiver: spl receive <outfile>");
        return;
    }

    if args[1].starts_with("-") {
        send_file_tcp(&args[2], &args[1][1..], key, port);
    } else if args[1] == "receive" {
        receive_file_tcp(&args[2], key, port);
    } else {
        println!("❌ Invalid command");
    }
}

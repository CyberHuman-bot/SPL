use std::fs::File;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::{SystemTime, UNIX_EPOCH};

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use rand::Rng;

// 2 MB chunks
const CHUNK_SIZE: usize = 2 * 1024 * 1024;
const SERVER_PORT: &str = "5001";

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
        stream.write_all(&len_bytes).unwrap(); // send length
        stream.write_all(&encrypted).unwrap(); // then encrypted data

        sent_bytes += n;
        print_progress(sent_bytes, size, start);
    }
    println!("\n✅ Transfer complete");
}

// ---- TCP Receiver ----
fn receive_file_tcp(outfile: &str) {
    let listener = TcpListener::bind(format!("0.0.0.0:{}", SERVER_PORT)).unwrap();
    println!("📥 Receiver ready on port {}, saving to {}", SERVER_PORT, outfile);

    let (mut stream, _) = listener.accept().unwrap();

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
        print_progress(got_bytes, got_bytes, start); // approximate total
    }

    println!("\n✅ File saved as {}", outfile);
}

// ---- CLI ----
fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        println!("Usage:\n  Sender: spl -<IP> <file>\n  Receiver: spl receive <outfile>");
        return;
    }

    if args[1].starts_with("-") {
        send_file_tcp(&args[2], &args[1][1..]);
    } else if args[1] == "receive" {
        receive_file_tcp(&args[2]);
    } else {
        println!("❌ Invalid command");
    }
}
// ---- End of file ----

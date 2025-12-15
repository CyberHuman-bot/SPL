use std::fs::File;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::net::TcpStream;
use std::time::{Instant};
use crate::crypto::{encrypt_chunk, decrypt_chunk};
use crate::utils::print_progress;
use crate::config::{SERVER_PORT, MAX_RETRIES, CHUNK_SIZE_BASE};

/// Send a file to the given IP
pub fn send_file(filename: &str, ip: &str, key: &[u8]) {
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

    // Send AES key first
    stream.write_all(&(key.len() as u32).to_be_bytes()).unwrap();
    stream.write_all(key).unwrap();

    let start = Instant::now();
    let mut buffer = vec![0u8; CHUNK_SIZE_BASE];
    let mut sent_bytes = 0;

    loop {
        let n = file.read(&mut buffer).unwrap();
        if n == 0 { break; }

        let mut retries = 0;
        loop {
            let encrypted = encrypt_chunk(key, &buffer[..n]);
            let len_bytes = (encrypted.len() as u32).to_be_bytes();

            if stream.write_all(&len_bytes).is_err() || stream.write_all(&encrypted).is_err() {
                if retries < MAX_RETRIES {
                    retries += 1;
                    eprintln!("⚠ Retry {} for chunk", retries);
                    continue;
                } else {
                    panic!("❌ Failed to send chunk after {} retries", MAX_RETRIES);
                }
            }
            break;
        }

        sent_bytes += n;
        print_progress(sent_bytes, size, start);
    }

    println!("\n✅ Transfer complete");
}

/// Receive a file and save it
pub fn receive_file(outfile: &str) {
    let listener = TcpListener::bind(format!("0.0.0.0:{}", SERVER_PORT)).unwrap();
    println!("📥 Receiver ready on port {}, saving to {}", SERVER_PORT, outfile);

    let (mut stream, addr) = listener.accept().unwrap();
    println!("✅ Connection from {}", addr);

    // Receive AES key
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).unwrap();
    let key_len = u32::from_be_bytes(len_buf) as usize;
    let mut key = vec![0u8; key_len];
    stream.read_exact(&mut key).unwrap();

    let mut file = File::create(outfile).unwrap();
    let start = Instant::now();
    let mut got_bytes = 0;

    loop {
        let mut len_buf = [0u8; 4];
        if stream.read_exact(&mut len_buf).is_err() { break; }
        let len = u32::from_be_bytes(len_buf) as usize;

        let mut encrypted = vec![0u8; len];
        if stream.read_exact(&mut encrypted).is_err() { break; }

        let mut retries = 0;
        loop {
            match decrypt_chunk(&key, &encrypted) {
                Ok(data) => {
                    file.write_all(&data).unwrap();
                    got_bytes += data.len();
                    print_progress(got_bytes, got_bytes, start);
                    break;
                },
                Err(_) => {
                    if retries < MAX_RETRIES {
                        retries += 1;
                        eprintln!("⚠ Retry {} for chunk decryption", retries);
                        continue;
                    } else {
                        panic!("❌ Decryption failed after {} retries", MAX_RETRIES);
                    }
                }
            }
        }
    }

    println!("\n✅ File saved as {}", outfile);
}

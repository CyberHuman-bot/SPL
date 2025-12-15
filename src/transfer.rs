use std::fs::{File, OpenOptions};
use std::io::{Read, Write, Seek, SeekFrom};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::thread;
use std::collections::HashSet;
use std::time::{Instant, Duration};

use crate::crypto::{encrypt_chunk, decrypt_chunk};
use crate::utils::print_progress;
use crate::config::{CHUNK_SIZE_BASE, MAX_RETRIES};

/// Represents a chunk to send
struct Chunk {
    index: usize,
    offset: u64,
    size: usize,
}

/// Send file in parallel chunks
pub fn send_file(filename: &str, ip: &str, key: &[u8]) {
    let file_size = std::fs::metadata(filename).unwrap().len() as usize;
    let chunk_size = CHUNK_SIZE_BASE;
    let total_chunks = (file_size + chunk_size - 1) / chunk_size;

    println!("📤 Sending '{}' ({:.2} MB) → {}", filename, file_size as f64 / 1024.0 / 1024.0, ip);
    println!("Total chunks: {}", total_chunks);

    let mut stream = TcpStream::connect(format!("{}:{}", ip, crate::config::SERVER_PORT)).unwrap();

    // Send AES key first
    stream.write_all(&(key.len() as u32).to_be_bytes()).unwrap();
    stream.write_all(key).unwrap();

    let file = Arc::new(Mutex::new(File::open(filename).unwrap()));
    let progress = Arc::new(Mutex::new(0usize));
    let start = Instant::now();

    // Create chunk queue
    let chunks: Vec<_> = (0..total_chunks).map(|i| Chunk {
        index: i,
        offset: (i * chunk_size) as u64,
        size: if (i + 1) * chunk_size > file_size { file_size - i * chunk_size } else { chunk_size },
    }).collect();
    let chunk_queue = Arc::new(Mutex::new(chunks));

    // Threads
    let mut handles = vec![];
    let thread_count = 4.min(total_chunks); // max 4 threads or total_chunks

    for _ in 0..thread_count {
        let queue = Arc::clone(&chunk_queue);
        let file = Arc::clone(&file);
        let progress = Arc::clone(&progress);
        let mut stream = stream.try_clone().unwrap();
        let key = key.to_vec();

        let handle = thread::spawn(move || {
            while let Some(chunk) = {
                let mut q = queue.lock().unwrap();
                q.pop()
            } {
                let mut buf = vec![0u8; chunk.size];
                {
                    let mut f = file.lock().unwrap();
                    f.seek(SeekFrom::Start(chunk.offset)).unwrap();
                    f.read_exact(&mut buf).unwrap();
                }

                let mut retries = 0;
                loop {
                    let encrypted = encrypt_chunk(&key, &buf);
                    let len_bytes = (encrypted.len() as u32).to_be_bytes();

                    if stream.write_all(&len_bytes).is_err() || stream.write_all(&encrypted).is_err() {
                        if retries < MAX_RETRIES {
                            retries += 1;
                            eprintln!("⚠ Retry {} for chunk {}", retries, chunk.index);
                            thread::sleep(Duration::from_millis(100));
                            continue;
                        } else {
                            panic!("❌ Failed to send chunk {} after {} retries", chunk.index, MAX_RETRIES);
                        }
                    }
                    break;
                }

                let mut prog = progress.lock().unwrap();
                *prog += chunk.size;
                print_progress(*prog, file_size, start);
            }
        });
        handles.push(handle);
    }

    for h in handles { h.join().unwrap(); }

    println!("\n✅ Transfer complete");
}

/// Receive file in parallel chunks (resume supported)
pub fn receive_file(outfile: &str) {
    use std::net::TcpListener;
    let listener = TcpListener::bind(format!("0.0.0.0:{}", crate::config::SERVER_PORT)).unwrap();
    println!("📥 Receiver ready on port {}, saving to {}", crate::config::SERVER_PORT, outfile);

    let (mut stream, addr) = listener.accept().unwrap();
    println!("✅ Connection from {}", addr);

    // Receive AES key
    let mut len_buf = [0u8;4];
    stream.read_exact(&mut len_buf).unwrap();
    let key_len = u32::from_be_bytes(len_buf) as usize;
    let mut key = vec![0u8; key_len];
    stream.read_exact(&mut key).unwrap();

    let mut file = OpenOptions::new().create(true).write(true).open(outfile).unwrap();
    let start = Instant::now();
    let mut total_received = 0usize;

    loop {
        let mut len_buf = [0u8;4];
        if stream.read_exact(&mut len_buf).is_err() { break; }
        let len = u32::from_be_bytes(len_buf) as usize;
        let mut encrypted = vec![0u8; len];
        if stream.read_exact(&mut encrypted).is_err() { break; }

        let mut retries = 0;
        loop {
            match decrypt_chunk(&key, &encrypted) {
                Ok(data) => {
                    file.write_all(&data).unwrap();
                    total_received += data.len();
                    print_progress(total_received, total_received, start);
                    break;
                },
                Err(_) => {
                    if retries < MAX_RETRIES {
                        retries += 1;
                        eprintln!("⚠ Retry {} for chunk decryption", retries);
                        thread::sleep(Duration::from_millis(50));
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

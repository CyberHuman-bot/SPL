use std::net::{UdpSocket, TcpListener, TcpStream};
use std::time::Duration;
use crate::config::{DISCOVERY_PORT, DISCOVERY_TIMEOUT, SERVER_PORT};
use std::collections::HashMap;
use std::thread;

pub fn discover_devices() -> Vec<String> {
    println!("🔍 Discovering devices...");
    let socket = UdpSocket::bind("0.0.0.0:0").unwrap();
    socket.set_read_timeout(Some(Duration::from_secs(DISCOVERY_TIMEOUT))).unwrap();

    socket.send_to(b"SPL_DISCOVER", format!("255.255.255.255:{}", DISCOVERY_PORT)).ok();
    let mut devices = HashMap::new();
    let mut buffer = [0u8; 1024];

    while let Ok((len, addr)) = socket.recv_from(&mut buffer) {
        if &buffer[..len] == b"SPL_HERE" {
            devices.insert(addr.ip().to_string(), addr);
        }
    }

    let mut device_list: Vec<_> = devices.keys().cloned().collect();
    device_list.sort();
    device_list
}

pub fn start_discovery_responder() {
    thread::spawn(|| {
        let socket = UdpSocket::bind(format!("0.0.0.0:{}", DISCOVERY_PORT)).unwrap();
        let mut buffer = [0u8; 1024];
        loop {
            if let Ok((len, _)) = socket.recv_from(&mut buffer) {
                if &buffer[..len] == b"SPL_DISCOVER" {
                    socket.send_to(b"SPL_HERE", format!("255.255.255.255:{}", DISCOVERY_PORT)).ok();
                }
            }
        }
    });
}

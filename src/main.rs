use clap::{Parser, Subcommand};
use rand::Rng;
use crate::network::{discover_devices, start_discovery_responder};
use crate::transfer::{send_file, receive_file};

mod crypto;
mod network;
mod transfer;
mod utils;
mod config;

/// SPL: Secure Package Loader
#[derive(Parser)]
#[command(author="Yaman", version="1.0", about="Secure file transfer tool")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Send a file to discovered devices
    Send {
        /// Path to the file to send
        file: String,
    },
    /// Receive a file
    Receive {
        /// Path to save the incoming file
        outfile: String,
    },
}

fn main() {
    let cli = Cli::parse();

    // Always start discovery responder so this device can be discovered
    start_discovery_responder();

    match cli.command {
        Commands::Send { file } => {
            // Discover devices on network
            let devices = discover_devices();
            if devices.is_empty() {
                println!("❌ No devices found on network");
                return;
            }

            println!("\n📱 Discovered devices:");
            for (i, ip) in devices.iter().enumerate() {
                println!("  {}: {}", i + 1, ip);
            }

            // For simplicity, select first device
            let ip = &devices[0];
            println!("\n🚀 Sending '{}' to {}", file, ip);

            // Generate random AES key for this transfer
            let key: [u8; 32] = rand::thread_rng().gen();
            send_file(&file, ip, &key);
        }

        Commands::Receive { outfile } => {
            println!("🖥 Ready to receive a file. Listening on port {}", crate::config::SERVER_PORT);
            receive_file(&outfile);
        }
    }
}

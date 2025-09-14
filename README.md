# SPL Rust

![Rust](https://img.shields.io/badge/rust-1.89.0-orange)
![Platform](https://img.shields.io/badge/platform-Windows%20|%20Linux-lightgrey)
![Build Status](https://img.shields.io/badge/build-passing-brightgreen)

**SPL** (Secure Package Loader) is a high-performance, encrypted file transfer tool written in **Rust**. It provides secure file transfers using **AES-256-GCM encryption** over TCP connections, designed for reliability and speed when transferring large files across networks.

---

## 🚀 Features

- 🔒 **Military-grade encryption**: AES-256-GCM encryption ensures your files stay secure
- 📦 **Chunked transfers**: Efficient handling of large files with optimized memory usage
- 🌐 **Cross-platform**: Native support for Windows, Linux, and macOS
- 💻 **Simple CLI interface**: Easy-to-use command-line tool for both sending and receiving
- ⚡ **High-speed performance**: Optimized TCP streaming with minimal overhead
- 🔑 **Automatic key management**: Secure key generation and configuration handling
- 📊 **Progress tracking**: Real-time transfer progress and speed monitoring
- 🛡️ **Error recovery**: Built-in retry mechanisms and connection resilience

---

## 📋 Table of Contents

- [Prerequisites](#prerequisites)
- [Installation](#installation)
- [Quick Start](#quick-start)
- [Usage](#usage)
- [Configuration](#configuration)
- [Examples](#examples)
- [Security](#security)
- [Troubleshooting](#troubleshooting)
- [Contributing](#contributing)
- [License](#license)

---

## 📦 Prerequisites

- **Rust**: Version 1.89.0 or newer ([Install Rust](https://rustup.rs/))
- **Cargo**: Rust package manager (included with Rust installation)
- **Network**: Both machines must be able to communicate over TCP on port 5001
- **Firewall**: Ensure port 5001 is open on the receiving machine

---

## 🔧 Installation

### Method 1: Build from Source

1. **Clone the repository:**
   ```bash
   git clone https://github.com/CyberHuman-bot/SPL.git
   cd SPL
   ```

2. **Build the project:**
   ```bash
   cargo build --release
   ```

3. **Install globally (optional):**
   ```bash
   cargo install --path .
   ```

The compiled binary will be available at:
- **Release build**: `target/release/spl_rust`
- **Global install**: Available as `spl_rust` in your PATH

### Method 2: Direct Installation from Git

```bash
cargo install --git https://github.com/CyberHuman-bot/SPL.git
```

---

## 🏃 Quick Start

### Sending a File

1. **Start the receiver** on the target machine:
   ```bash
   ./spl_rust --receive
   ```

2. **Send the file** from the source machine:
   ```bash
   ./spl_rust --send <IP_ADDRESS> <FILE_PATH>
   ```

### Example Transfer

```bash
# On receiver (192.168.1.100)
./spl_rust --receive

# On sender
./spl_rust --send 192.168.1.100 ./document.pdf
```

---

## 📖 Usage

### Command Line Options

```bash
SPL - Secure Package Loader

USAGE:
    spl_rust [OPTIONS] <SUBCOMMAND>

OPTIONS:
    -p, --port <PORT>         Port to use for transfer [default: 8080]
    -v, --verbose             Enable verbose output
    -q, --quiet               Suppress non-essential output
    -h, --help                Print help information
    -V, --version             Print version information

SUBCOMMANDS:
    send <IP> <FILE>          Send file to specified IP address
    receive                   Start receiver mode
    config                    Generate or modify configuration
    help                      Print this message or the help of subcommands
```

### Detailed Command Usage

#### Sending Files

```bash
# Basic file send
./spl_rust send 192.168.1.100 ./myfile.zip

# Send with custom port
./spl_rust --port 9000 send 192.168.1.100 ./large_file.iso

# Verbose mode for debugging
./spl_rust --verbose send 192.168.1.100 ./data.tar.gz
```

#### Receiving Files

```bash
# Start receiver on default port (8080)
./spl_rust receive

# Receive on custom port
./spl_rust --port 9000 receive

# Quiet mode (minimal output)
./spl_rust --quiet receive
```

---

## ⚙️ Configuration

SPL automatically generates a configuration file on first run. You can customize settings by editing the config file or using the config command:

```bash
# Generate new configuration
./spl_rust config --generate

# View current configuration
./spl_rust config --show

# Set custom encryption key
./spl_rust config --set-key <KEY>
```

### Configuration File Location

- **Linux/macOS**: `~/.config/spl/config.toml`
- **Windows**: `%APPDATA%\SPL\config.toml`

---

## 💡 Examples

### Basic File Transfer

```bash
# Terminal 1 (Receiver - 192.168.1.100)
./spl_rust receive
# Output: Listening on 192.168.1.100:8080...

# Terminal 2 (Sender)
./spl_rust send 192.168.1.100 ./presentation.pptx
# Output: Transferring presentation.pptx... [████████████████████] 100% (5.2 MB/s)
```

### Large File Transfer with Custom Port

```bash
# Receiver
./spl_rust --port 9999 receive

# Sender
./spl_rust --port 9999 send 192.168.1.100 ./backup.tar.gz
```

### Batch Operations

```bash
# Send multiple files (using shell loop)
for file in *.pdf; do
    ./spl_rust send 192.168.1.100 "$file"
    sleep 2  # Wait between transfers
done
```

---

## 🔐 Security

SPL implements several security measures to protect your file transfers:

- **AES-256-GCM Encryption**: Industry-standard encryption with authenticated encryption
- **Secure Key Generation**: Cryptographically secure random key generation
- **Perfect Forward Secrecy**: New session keys for each transfer
- **Integrity Verification**: Built-in checksums and authentication tags
- **No Key Storage**: Encryption keys are never stored on disk

### Security Best Practices

- Always use SPL over trusted networks when possible
- Regularly update to the latest version
- Verify file integrity after large transfers
- Use strong, unique passwords for any additional authentication layers

---

## 🐛 Troubleshooting

### Common Issues

**Connection Refused**
```bash
Error: Connection refused (os error 111)
```
- Ensure the receiver is running before sending
- Check firewall settings on both machines
- Verify the IP address and port are correct

**Permission Denied**
```bash
Error: Permission denied (os error 13)
```
- Check file permissions on the source file
- Ensure write permissions in the destination directory
- Try running with appropriate privileges

**Large File Transfers Failing**
- Check available disk space on receiving machine
- Verify network stability
- Consider using a wired connection for very large files

### Debug Mode

Enable verbose logging to diagnose issues:

```bash
./spl_rust --verbose send 192.168.1.100 ./file.txt
```

---

## 🤝 Contributing

We welcome contributions! Please see our [Contributing Guide](CONTRIBUTING.md) for details.

### Development Setup

```bash
git clone https://github.com/CyberHuman-bot/SPL.git
cd SPL
cargo build
cargo test
```

### Running Tests

```bash
# Run all tests
cargo test

# Run with coverage
cargo test --all-features

# Run specific test
cargo test test_encryption
```

---

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

---

## 🙏 Acknowledgments

- Built with [Rust](https://www.rust-lang.org/) and the amazing Rust ecosystem
- Encryption provided by [ring](https://github.com/briansmith/ring) cryptographic library
- Special thanks to the Rust community for excellent documentation and support

---

## 📞 Support

- **Issues**: [GitHub Issues](https://github.com/CyberHuman-bot/SPL/issues)
- **Discussions**: [GitHub Discussions](https://github.com/CyberHuman-bot/SPL/discussions)
- **Email**: [support@example.com](mailto:support@example.com)

---

<div align="center">

**[⬆ Back to Top](#spl-rust)**

Made with ❤️ and 🦀 by the SPL Team

</div>

use std::time::Instant;
use std::io::{self, Write};

pub fn print_progress(transferred: usize, total: usize, start: Instant) {
    let percent = if total > 0 {
        (transferred as f64 / total as f64) * 100.0
    } else { 0.0 };
    let elapsed = start.elapsed().as_secs_f64().max(0.01);
    let speed = transferred as f64 / (1024.0 * 1024.0) / elapsed;
    let bar_len = 30;
    let filled = ((bar_len as f64) * transferred as f64 / (total as f64)).round() as usize;
    let bar = "█".repeat(filled) + &"-".repeat(bar_len - filled);
    print!("\r[{}] {:.1}% | {:.2} MB/s", bar, percent, speed);
    io::stdout().flush().unwrap();
}

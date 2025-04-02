use std::fs;
use notify_rust::{Notification, Urgency};

pub fn print_mem() -> String {
    let meminfo = fs::read_to_string("/proc/meminfo").unwrap_or_default();
    let mut mem_free = 0;
    for line in meminfo.lines() {
        if line.starts_with("MemAvailable:") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                mem_free = parts[1].parse::<u64>().unwrap_or(0) / 1024;
            }
        }
    }
    if mem_free < 2000 {
        let _ = Notification::new()
            .summary("内存警告")
            .body(&format!("可用内存：{}MB", mem_free))
            .urgency(Urgency::Low)
            .show();
    }
    format!("{}", mem_free)
}


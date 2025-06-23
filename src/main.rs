// statbar-display-rs/src/main.rs
use std::fs;
use std::io;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};
use sysinfo::{System, SystemExt}; // 移除了未使用的 CpuExt
use chrono::Local;

const NET_LOG: &str = "/dev/shm/statbar_netlog";
const CPU_LOG: &str = "/dev/shm/statbar_cpulog";
const SLEEP_INTERVAL: u64 = 2;

struct NetStats {
    rx_bytes: u64,
    tx_bytes: u64,
}

impl NetStats {
    fn load() -> io::Result<Self> {
        let data = fs::read_to_string(NET_LOG)?;
        let parts: Vec<&str> = data.split_whitespace().collect();
        if parts.len() >= 2 {
            Ok(NetStats {
                rx_bytes: parts[0].parse().unwrap_or(0),
                tx_bytes: parts[1].parse().unwrap_or(0),
            })
        } else {
            Ok(NetStats {
                rx_bytes: 0,
                tx_bytes: 0,
            })
        }
    }

    fn save(&self) -> io::Result<()> {
        fs::write(NET_LOG, format!("{} {}", self.rx_bytes, self.tx_bytes))
    }
}

struct CpuStats {
    total: u64,
    idle: u64,
}

impl CpuStats {
    fn load() -> io::Result<Self> {
        let data = fs::read_to_string(CPU_LOG)?;
        let parts: Vec<&str> = data.split_whitespace().collect();
        if parts.len() >= 2 {
            Ok(CpuStats {
                total: parts[0].parse().unwrap_or(0),
                idle: parts[1].parse().unwrap_or(0),
            })
        } else {
            Ok(CpuStats {
                total: 0,
                idle: 0,
            })
        }
    }

    fn save(&self) -> io::Result<()> {
        fs::write(CPU_LOG, format!("{} {}", self.total, self.idle))
    }
}

fn get_network_speed(prev: &NetStats) -> io::Result<(u64, u64)> {
    let mut rx_total = 0;
    let mut tx_total = 0;
    
    for entry in fs::read_dir("/sys/class/net")? {
        let entry = entry?;
        let path = entry.path();
        if let Some(iface) = path.file_name().and_then(|n| n.to_str()) {
            if iface.starts_with("en") || iface.starts_with("eth") || iface.starts_with("wl") {
                let rx_path = path.join("statistics/rx_bytes");
                let tx_path = path.join("statistics/tx_bytes");
                
                if rx_path.exists() {
                    if let Ok(content) = fs::read_to_string(rx_path) {
                        rx_total += content.trim().parse::<u64>().unwrap_or(0);
                    }
                }
                if tx_path.exists() {
                    if let Ok(content) = fs::read_to_string(tx_path) {
                        tx_total += content.trim().parse::<u64>().unwrap_or(0);
                    }
                }
            }
        }
    }
    
    let rx_speed = (rx_total - prev.rx_bytes) * 8 / SLEEP_INTERVAL / 1000; // Kbps
    let tx_speed = (tx_total - prev.tx_bytes) * 8 / SLEEP_INTERVAL / 1000; // Kbps
    
    Ok((rx_speed, tx_speed))
}

fn get_memory_usage() -> u64 {
    if let Ok(meminfo) = fs::read_to_string("/proc/meminfo") {
        for line in meminfo.lines() {
            if line.starts_with("MemAvailable:") {
                if let Some(kb) = line.split_whitespace().nth(1) {
                    return kb.parse().unwrap_or(0) / 1024; // MB
                }
            }
        }
    }
    0
}

fn get_cpu_temperature() -> Option<i32> {
    // 尝试从 sysfs 获取温度
    if let Ok(entries) = fs::read_dir("/sys/devices/platform/coretemp.0/hwmon") {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Ok(temp_entries) = fs::read_dir(&path) {
                    for temp_entry in temp_entries.flatten() {
                        let temp_path = temp_entry.path();
                        if let Some(name) = temp_path.file_name().and_then(|n| n.to_str()) {
                            if name.starts_with("temp") && name.ends_with("_input") {
                                if let Ok(content) = fs::read_to_string(&temp_path) {
                                    return content.trim().parse::<i32>().ok().map(|t| t / 1000);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    
    // 回退到 sensors 命令
    let output = std::process::Command::new("sensors")
        .arg("-A")
        .arg("coretemp-isa-0000")
        .output()
        .ok()?;
    
    if output.status.success() {
        let output_str = String::from_utf8_lossy(&output.stdout);
        for line in output_str.lines() {
            if line.contains("Package id 0:") {
                let parts: Vec<&str> = line.split('+').collect();
                if parts.len() > 1 {
                    return parts[1].split('.').next()?.parse::<i32>().ok();
                }
            }
        }
    }
    
    None
}

fn get_battery_status() -> (Option<u8>, Option<String>) {
    let mut capacity = None;
    let mut status = None;
    
    if let Ok(entries) = fs::read_dir("/sys/class/power_supply") {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with("BAT") {
                    // 读取电池容量
                    if capacity.is_none() {
                        if let Ok(content) = fs::read_to_string(path.join("capacity")) {
                            capacity = content.trim().parse::<u8>().ok();
                        }
                    }
                    
                    // 读取电池状态
                    if status.is_none() {
                        if let Ok(content) = fs::read_to_string(path.join("status")) {
                            status = Some(content.trim().to_string());
                        }
                    }
                    
                    // 如果两者都已获取，提前退出
                    if capacity.is_some() && status.is_some() {
                        break;
                    }
                }
            }
        }
    }
    
    (capacity, status)
}

fn get_cpu_usage(prev: &CpuStats) -> io::Result<u8> {
    let cpu_line = fs::read_to_string("/proc/stat")?;
    let cpu_line = cpu_line.lines().next().ok_or_else(|| io::Error::new(io::ErrorKind::Other, "No CPU line"))?;
    
    let values: Vec<u64> = cpu_line
        .split_whitespace()
        .skip(1)
        .filter_map(|s| s.parse().ok())
        .collect();
    
    if values.len() < 4 {
        return Err(io::Error::new(io::ErrorKind::Other, "Invalid CPU stats"));
    }
    
    let total: u64 = values.iter().sum();
    let idle = values[3];
    
    let delta_total = total - prev.total;
    let delta_idle = idle - prev.idle;
    
    if delta_total > 0 {
        let usage = ((delta_total - delta_idle) * 100 / delta_total) as u8;
        Ok(usage)
    } else {
        Ok(0)
    }
}

fn init_logs() -> io::Result<()> {
    if !Path::new(CPU_LOG).exists() {
        let cpu_line = fs::read_to_string("/proc/stat")?;
        let cpu_line = cpu_line.lines().next().ok_or_else(|| io::Error::new(io::ErrorKind::Other, "No CPU line"))?;
        
        let values: Vec<u64> = cpu_line
            .split_whitespace()
            .skip(1)
            .filter_map(|s| s.parse().ok())
            .collect();
        
        if values.len() < 4 {
            return Err(io::Error::new(io::ErrorKind::Other, "Invalid CPU stats"));
        }
        
        let total: u64 = values.iter().sum();
        let idle = values[3];
        
        fs::write(CPU_LOG, format!("{} {}", total, idle))?;
    }
    
    if !Path::new(NET_LOG).exists() {
        fs::write(NET_LOG, "0 0")?;
    }
    
    Ok(())
}

fn main() -> io::Result<()> {
    init_logs()?;
    
    let mut sys = System::new();
    let mut prev_net = NetStats::load().unwrap_or(NetStats { rx_bytes: 0, tx_bytes: 0 });
    let mut prev_cpu = CpuStats::load().unwrap_or(CpuStats { total: 0, idle: 0 });
    
    // 初始网络统计
    let _ = get_network_speed(&prev_net)?;
    
    loop {
        let start = Instant::now();
        
        // 获取内存使用
        let mem_usage = get_memory_usage();
        
        // 获取CPU使用率
        let cpu_usage = get_cpu_usage(&prev_cpu).unwrap_or(0);
        
        // 保存当前CPU状态
        if let Ok(current_cpu) = get_current_cpu_stats() {
            prev_cpu = current_cpu;
            prev_cpu.save().ok();
        }
        
        // 获取温度
        let temp = get_cpu_temperature().map(|t| format!("{}°", t)).unwrap_or_else(|| "N/A".to_string());
        
        // 获取网络速度
        let (rx_speed, tx_speed) = get_network_speed(&prev_net).unwrap_or((0, 0));
        prev_net = get_current_net_stats().unwrap_or(prev_net);
        prev_net.save().ok();
        
        // 获取电池状态
        let (bat_capacity, bat_status) = get_battery_status();
        let bat_display = match (bat_capacity, bat_status.as_deref()) {
            (Some(cap), Some("Charging")) => format!("⚡{}%", cap),
            (Some(cap), _) => format!("{}%", cap),
            _ => "N/A".to_string(),
        };
        
        // 获取时间
        let time = Local::now().format("%H:%M").to_string();
        
        // 更新系统信息
        sys.refresh_cpu();
        sys.refresh_memory();
        
        // 输出状态
        println!(
            " {} {}% {} ↑{}K ↓{}K {} {} ",
            mem_usage,
            cpu_usage,
            temp,
            tx_speed,
            rx_speed,
            bat_display,
            time
        );
        
        // 精确睡眠
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(SLEEP_INTERVAL) {
            thread::sleep(Duration::from_secs(SLEEP_INTERVAL) - elapsed);
        }
    }
}

// 获取当前网络统计
fn get_current_net_stats() -> io::Result<NetStats> {
    let mut rx_total = 0;
    let mut tx_total = 0;
    
    for entry in fs::read_dir("/sys/class/net")? {
        let entry = entry?;
        let path = entry.path();
        if let Some(iface) = path.file_name().and_then(|n| n.to_str()) {
            if iface.starts_with("en") || iface.starts_with("eth") || iface.starts_with("wl") {
                let rx_path = path.join("statistics/rx_bytes");
                let tx_path = path.join("statistics/tx_bytes");
                
                if rx_path.exists() {
                    if let Ok(content) = fs::read_to_string(rx_path) {
                        rx_total += content.trim().parse::<u64>().unwrap_or(0);
                    }
                }
                if tx_path.exists() {
                    if let Ok(content) = fs::read_to_string(tx_path) {
                        tx_total += content.trim().parse::<u64>().unwrap_or(0);
                    }
                }
            }
        }
    }
    
    Ok(NetStats {
        rx_bytes: rx_total,
        tx_bytes: tx_total,
    })
}

// 获取当前CPU统计
fn get_current_cpu_stats() -> io::Result<CpuStats> {
    let cpu_line = fs::read_to_string("/proc/stat")?;
    let cpu_line = cpu_line.lines().next().ok_or_else(|| io::Error::new(io::ErrorKind::Other, "No CPU line"))?;
    
    let values: Vec<u64> = cpu_line
        .split_whitespace()
        .skip(1)
        .filter_map(|s| s.parse().ok())
        .collect();
    
    if values.len() < 4 {
        return Err(io::Error::new(io::ErrorKind::Other, "Invalid CPU stats"));
    }
    
    let total: u64 = values.iter().sum();
    let idle = values[3];
    
    Ok(CpuStats { total, idle })
}

use glob::glob;
use std::fs;
use std::path::Path;

pub fn print_network_speed() -> String {
    let logfile = "/dev/shm/netlog";
    if !Path::new(logfile).exists() {
        let _ = fs::write(logfile, "0 0");
    }
    let content = fs::read_to_string(logfile).unwrap_or_else(|_| "0 0".to_string());
    let mut parts = content.split_whitespace();
    let rxprev: u64 = parts.next().unwrap_or("0").parse().unwrap_or(0);
    let txprev: u64 = parts.next().unwrap_or("0").parse().unwrap_or(0);
    
    let mut rxcurrent = 0u64;
    let mut txcurrent = 0u64;
    for entry in glob("/sys/class/net/[ew]*").unwrap().filter_map(Result::ok) {
        if entry.is_dir() {
            let _iface = entry.file_name().unwrap().to_string_lossy();
            let rx_path = entry.join("statistics/rx_bytes");
            let tx_path = entry.join("statistics/tx_bytes");
            if let Ok(rx_str) = fs::read_to_string(&rx_path) {
                rxcurrent += rx_str.trim().parse::<u64>().unwrap_or(0);
            }
            if let Ok(tx_str) = fs::read_to_string(&tx_path) {
                txcurrent += tx_str.trim().parse::<u64>().unwrap_or(0);
            }
        }
    }
    
    let diff_rx = rxcurrent.saturating_sub(rxprev) as f64;
    let diff_tx = txcurrent.saturating_sub(txprev) as f64;
    let rx_mb = diff_rx / 1e6;
    let tx_mb = diff_tx / 1e6;
    let _ = fs::write(logfile, format!("{} {}", rxcurrent, txcurrent));
    
    // 添加 BOXCHARS 效果：以 10 MB/s 为满量程（可根据实际情况调整）
    const BOXCHARS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    let graph_rx = {
        let ratio = (rx_mb / 10.0).min(1.0);
        let idx = (ratio * 7.0).round() as usize;
        BOXCHARS[idx]
    };
    let graph_tx = {
        let ratio = (tx_mb / 10.0).min(1.0);
        let idx = (ratio * 7.0).round() as usize;
        BOXCHARS[idx]
    };

    format!("{} {:.2} ↓  {} {:.2} ↑", graph_rx, rx_mb, graph_tx, tx_mb)
}


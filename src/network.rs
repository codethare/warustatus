use std::fs;
use std::path::Path;

pub fn print_network_speed() -> String {
    let logfile = "/dev/shm/netlog";

    // 如果日志文件不存在，则创建
    if !Path::new(logfile).exists() {
        let _ = fs::write(logfile, "0 0");
    }

    let content = fs::read_to_string(logfile).unwrap_or_else(|_| "0 0".to_string());
    let mut parts = content.split_whitespace();
    let rxprev: u64 = parts.next().unwrap_or("0").parse().unwrap_or(0);
    let txprev: u64 = parts.next().unwrap_or("0").parse().unwrap_or(0);

    let mut rxcurrent = 0u64;
    let mut txcurrent = 0u64;

    // 遍历 /sys/class/net 目录
    if let Ok(entries) = fs::read_dir("/sys/class/net") {
        for entry in entries.flatten() {
            // 获取接口名称，确保为 UTF-8 字符串
            if let Ok(iface) = entry.file_name().into_string() {
                // 仅处理名称以 'e' 或 'w' 开头的接口（例如 ethernet 或 wifi）
                if iface.starts_with('e') || iface.starts_with('w') {
                    let rx_path = entry.path().join("statistics/rx_bytes");
                    let tx_path = entry.path().join("statistics/tx_bytes");
                    if Path::new(&rx_path).exists() {
                        if let Ok(rx_str) = fs::read_to_string(&rx_path) {
                            rxcurrent += rx_str.trim().parse::<u64>().unwrap_or(0);
                        }
                    }
                    if Path::new(&tx_path).exists() {
                        if let Ok(tx_str) = fs::read_to_string(&tx_path) {
                            txcurrent += tx_str.trim().parse::<u64>().unwrap_or(0);
                        }
                    }
                }
            }
        }
    }

    let diff_rx = rxcurrent.saturating_sub(rxprev) as f64;
    let diff_tx = txcurrent.saturating_sub(txprev) as f64;
    let rx_mb = diff_rx / 1e6;
    let tx_mb = diff_tx / 1e6;
    let _ = fs::write(logfile, format!("{} {}", rxcurrent, txcurrent));

    format!("{:.2} ↓ {:.2} ↑", rx_mb, tx_mb)
}


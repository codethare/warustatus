use std::{fs, time::Instant};

#[derive(Clone)]

pub struct NetworkStats {
    pub rx_mbps: f64,
    pub tx_mbps: f64,
    last_rx: u64,
    last_tx: u64,
    last_time: Instant,
}

impl NetworkStats {
    pub fn new() -> Self {
        Self {
            rx_mbps: 0.0,
            tx_mbps: 0.0,
            last_rx: 0,
            last_tx: 0,
            last_time: Instant::now(),
        }
    }

    pub fn update(&mut self) {
        let (rx, tx) = self.read_counters();
        let elapsed = self.last_time.elapsed().as_secs_f64();

        self.rx_mbps = (rx - self.last_rx) as f64 / 1_048_576.0 / elapsed.max(0.1);
        self.tx_mbps = (tx - self.last_tx) as f64 / 1_048_576.0 / elapsed.max(0.1);

        self.last_rx = rx;
        self.last_tx = tx;
        self.last_time = Instant::now();
    }

    fn read_counters(&self) -> (u64, u64) {
        let mut rx = 0;
        let mut tx = 0;

        if let Ok(dir) = fs::read_dir("/sys/class/net") {
            for entry in dir.filter_map(Result::ok) {
                let path = entry.path();
                if path.is_dir() {
                    let iface = path.file_name().unwrap().to_string_lossy();
                    if iface.starts_with("lo") {
                        continue;
                    }

                    if let Ok(rx_bytes) = fs::read_to_string(path.join("statistics/rx_bytes")) {
                        rx += rx_bytes.trim().parse::<u64>().unwrap_or(0);
                    }
                    if let Ok(tx_bytes) = fs::read_to_string(path.join("statistics/tx_bytes")) {
                        tx += tx_bytes.trim().parse::<u64>().unwrap_or(0);
                    }
                }
            }
        }

        (rx, tx)
    }
}

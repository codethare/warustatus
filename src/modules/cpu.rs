use std::fs;

pub struct CpuLoad {
    prev_idle: u64,
    prev_total: u64,
}

impl CpuLoad {
    pub fn new() -> Result<Self, std::io::Error> {
        let (idle, total) = Self::read_stat()?;
        Ok(Self { prev_idle: idle, prev_total: total })
    }

    pub fn update(&mut self) -> Result<f64, std::io::Error> {
        let (idle, total) = Self::read_stat()?;
        let idle_delta = idle - self.prev_idle;
        let total_delta = total - self.prev_total;

        self.prev_idle = idle;
        self.prev_total = total;

        Ok(100.0 * (1.0 - (idle_delta as f64 / total_delta as f64)))
    }

    fn read_stat() -> Result<(u64, u64), std::io::Error> {
        let content = fs::read_to_string("/proc/stat")?;
        let line = content.lines().next().ok_or(std::io::ErrorKind::InvalidData)?;
        
        let values: Vec<u64> = line.split_whitespace()
            .skip(1)
            .filter_map(|s| s.parse().ok())
            .collect();

        let idle = values.get(3).copied().unwrap_or(0) + values.get(4).copied().unwrap_or(0);
        let total = values.iter().sum();
        
        Ok((idle, total))
    }
}

pub struct CpuTemp {
    pub celsius: f32,
}

impl CpuTemp {
    pub fn now() -> Self {
        let mut max_temp: f32 = 0.0;
        if let Ok(dir) = fs::read_dir("/sys/class/thermal") {
            for entry in dir.filter_map(Result::ok) {
                let path = entry.path().join("temp");
                if let Ok(temp) = fs::read_to_string(path) {
                    if let Ok(t) = temp.trim().parse::<f32>() {
                        max_temp = max_temp.max(t / 1000.0);
                    }
                }
            }
        }
        Self { celsius: max_temp }
    }
}

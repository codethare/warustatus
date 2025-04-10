use std::fs;

// 添加 Clone, Debug, Default
#[derive(Clone, Debug, Default)]
pub struct MemoryInfo {
    available_mb: u64, // 改为 MB 更常用
}

impl MemoryInfo {
    pub fn now() -> Self {
        let mut available = 0;

        if let Ok(content) = fs::read_to_string("/proc/meminfo") {
            for line in content.lines() {
                if line.starts_with("MemAvailable:") {
                    // MemAvailable 单位是 kB
                    if let Some(value) = line.split_whitespace().nth(1) {
                        // kB -> MB
                        available = value.parse::<u64>().unwrap_or(0) / 1024;
                    }
                    break;
                }
            }
        }

        Self { available_mb: available }
    }

    // 提供获取 MB 的方法
    pub fn available_mb(&self) -> u64 {
        self.available_mb
    }

    // (可选) 提供获取 GB 的方法 (浮点数)
    pub fn available_gb(&self) -> f64 {
       (self.available_mb as f64) / 1024.0
    }
}

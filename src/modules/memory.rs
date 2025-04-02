use std::fs;

pub struct MemoryInfo {
    available: u64,
}

impl MemoryInfo {
    pub fn now() -> Self {
        let mut available = 0;

        if let Ok(content) = fs::read_to_string("/proc/meminfo") {
            for line in content.lines() {
                if line.starts_with("MemAvailable:") {
                    if let Some(value) = line.split_whitespace().nth(1) {
                        available = value.parse::<u64>().unwrap_or(0) / 1024;
                    }
                    break; // 只读取一次后就跳出循环，提高效率
                }
            }
        }

        Self { available }
    }

    pub fn available_gb(&self) -> u64 {
        self.available // 直接转换为整数，去除小数点
    }
}


use std::fs;

#[derive(Clone)]

// --- CpuLoad 保持不变 ---
pub struct CpuLoad {
    prev_idle: u64,
    prev_total: u64,
}

impl CpuLoad {
    pub fn new() -> Result<Self, std::io::Error> {
        let (idle, total) = Self::read_stat()?;
        Ok(Self {
            prev_idle: idle,
            prev_total: total,
        })
    }

    pub fn update(&mut self) -> Result<f64, std::io::Error> {
        let (idle, total) = Self::read_stat()?;
        // 防止除以零
        let total_delta = total.saturating_sub(self.prev_total);
        if total_delta == 0 {
            return Ok(0.0); // 如果总时间没有变化，返回 0% 使用率
        }
        let idle_delta = idle.saturating_sub(self.prev_idle);

        self.prev_idle = idle;
        self.prev_total = total;

        // 计算使用率，确保不小于 0
        let usage = 1.0 - (idle_delta as f64 / total_delta as f64);
        Ok((usage * 100.0).max(0.0))
    }

    fn read_stat() -> Result<(u64, u64), std::io::Error> {
        let content = fs::read_to_string("/proc/stat")?;
        let line = content
            .lines()
            .next()
            .ok_or(std::io::ErrorKind::InvalidData)?;

        let values: Vec<u64> = line
            .split_whitespace()
            .skip(1) // 跳过 "cpu"
            .filter_map(|s| s.parse().ok())
            .collect();

        // idle = idle + iowait
        let idle = values.get(3).copied().unwrap_or(0) + values.get(4).copied().unwrap_or(0);
        let total: u64 = values.iter().sum();

        Ok((idle, total))
    }
}
// --- CpuTemp 添加 Clone, Debug, Default ---
#[derive(Clone, Debug, Default)]
pub struct CpuTemp {
    pub celsius: f32,
}

impl CpuTemp {
    pub fn now() -> Self {
        let mut max_temp: f32 = 0.0;
        let mut found_temp = false; // 标记是否找到任何温度读数
        if let Ok(dir) = fs::read_dir("/sys/class/thermal") {
            for entry in dir.filter_map(Result::ok) {
                // 通常 thermal_zone* 是 CPU 温度
                if entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("thermal_zone")
                {
                    let path = entry.path().join("temp");
                    // 检查 type 文件确定是否是 CPU 相关温度 (可选，增加准确性)
                    let type_path = entry.path().join("type");
                    let is_cpu_temp = fs::read_to_string(type_path)
                        .map(|s| s.contains("x86_pkg_temp") || s.contains("cpu")) // 根据实际情况调整
                        .unwrap_or(false);

                    if !is_cpu_temp {
                        continue;
                    } // 如果类型不匹配，跳过

                    if let Ok(temp) = fs::read_to_string(&path) {
                        // 使用引用避免所有权问题
                        if let Ok(t) = temp.trim().parse::<f32>() {
                            max_temp = max_temp.max(t / 1000.0);
                            found_temp = true; // 至少找到了一个读数
                        }
                    }
                }
            }
        }
        // 如果没有找到温度，返回一个特殊值或保持 0.0
        if !found_temp {
            // 可以选择返回一个特定的值，比如 f32::NAN，并在打印时处理
            return Self { celsius: -1.0 }; // 用 -1.0 表示未找到
        }
        Self { celsius: max_temp }
    }
}

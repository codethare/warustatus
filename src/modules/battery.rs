use std::{fmt, fs, path::Path};

// 添加 Clone, Debug, Default
#[derive(Clone, Debug, Default)]
pub struct BatteryInfo {
    capacity: u8,
    status: String,
}

impl BatteryInfo {
    pub fn now() -> Self {
        let mut capacity = 0;
        let mut status = "N/A".to_string(); // 默认值

        if Path::new("/sys/class/power_supply/BAT0").exists() {
            if let Ok(cap) = fs::read_to_string("/sys/class/power_supply/BAT0/capacity") {
                capacity = cap.trim().parse().unwrap_or(0);
            }
            if let Ok(stat) = fs::read_to_string("/sys/class/power_supply/BAT0/status") {
                status = match stat.trim() {
                    "Charging" => "+".to_string(),
                    "Discharging" => "".to_string(),
                    "Full" => "",
                    _ => "POWER".to_string(), // 其他状态如 "Not charging" 等归为 POWER
                };
            } else {
                status = "N/A".to_string(); // 如果读取失败
            }
        } else {
            status = "NO BATT".to_string(); // 如果电池路径不存在
        }

        Self { capacity, status }
    }
}

impl fmt::Display for BatteryInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.status == "NO BATT" || self.status == "N/A" {
            write!(f, "{}", self.status) // 如果没有电池或状态未知，只显示状态
        } else {
            write!(f, "{}% {}", self.capacity, self.status)
        }
    }
}

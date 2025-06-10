use std::{fmt, fs, path::Path};

// 添加 Clone, Debug, Default
#[derive(Clone, Debug, Default)]
pub struct BatteryInfo {
    capacity: u8,
    status: String,
}

impl BatteryInfo {
    pub fn now() -> Result<Self, std::io::Error> {
        let mut capacity = 0;
        let mut status = "N/A".to_string(); // 默认值

        if Path::new("/sys/class/power_supply/BAT0").exists() {
            if let Ok(cap_str) = fs::read_to_string("/sys/class/power_supply/BAT0/capacity") {
                capacity = cap_str.trim().parse().unwrap_or(0);
            }

            if let Ok(stat_str) = fs::read_to_string("/sys/class/power_supply/BAT0/status") {
                status = match stat_str.trim() {
                    "Charging" => "⚡".to_string(), // 使用图标更直观
                    "Discharging" => "🔋".to_string(),
                    "Full" => "🔌".to_string(),
                    _ => "❔".to_string(), // 其他状态如 "Not charging" 等
                };
            }
        } else {
            status = "NO BATT".to_string(); // 如果电池路径不存在
        }

        Ok(Self { capacity, status })
    }
}


impl fmt::Display for BatteryInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.status == "NO BATT" || self.status == "N/A" {
            write!(f, "{}", self.status) // 如果没有电池或状态未知，只显示状态
        } else {
            // 只有当状态不是图标时，才添加空格
            write!(f, "{}% {}", self.capacity, self.status)
        }
    }
}

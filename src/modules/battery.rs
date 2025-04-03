use std::{fmt, fs, path::Path};

pub struct BatteryInfo {
    capacity: u8,
    status: String,
}

impl BatteryInfo {
    pub fn now() -> Self {
        let mut capacity = 0;
        let mut status = "Unknown".into();

        if Path::new("/sys/class/power_supply/BAT0").exists() {
            if let Ok(cap) = fs::read_to_string("/sys/class/power_supply/BAT0/capacity") {
                capacity = cap.trim().parse().unwrap_or(0);
            }
            if let Ok(stat) = fs::read_to_string("/sys/class/power_supply/BAT0/status") {
                status = match stat.trim() {
                    "Charging" => "CHARGING",
                    "Discharging" => "DISCHARGING",
                    _ => "POWER",
                }.to_string();
            }
        }

        Self { capacity, status }
    }
}

impl fmt::Display for BatteryInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}% {}", self.capacity, self.status)
    }
}

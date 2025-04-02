use std::fs;
use std::path::Path;
use notify_rust::{Notification, Urgency};

pub fn print_bat() -> String {
    let bat_path = "/sys/class/power_supply/BAT0";
    if !Path::new(bat_path).is_dir() {
        return "N/A".to_string();
    }
    let capacity_path = format!("{}/capacity", bat_path);
    let status_path = format!("{}/status", bat_path);
    let charge_str = fs::read_to_string(&capacity_path).unwrap_or_else(|_| "0".to_string());
    let status = fs::read_to_string(&status_path).unwrap_or_default();
    let charge = charge_str.trim().parse::<i32>().unwrap_or(0);
    let status_trimmed = status.trim();
    if status_trimmed == "Discharging" {
        if charge <= 6 {
            let _ = Notification::new()
                .summary("电量警报")
                .body(&format!("剩余电量：{}%", charge))
                .urgency(Urgency::Critical)
                .show();
        } else if charge <= 15 {
            let _ = Notification::new()
                .summary("电量提示")
                .body(&format!("剩余电量：{}%", charge))
                .urgency(Urgency::Low)
                .show();
        }
    }
    if status_trimmed == "Full" {
        "Full".to_string()
    } else {
        format!("{}%%", charge)
    }
}


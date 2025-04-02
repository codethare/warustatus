use serde_json::Value;
use std::process::Command;

pub fn print_ip_address() -> String {
    let output = Command::new("ip")
        .args(&["-j", "route", "get", "8.8.8.8"])
        .output();
    if output.is_err() || !output.as_ref().unwrap().status.success() {
        return "N/A".to_string();
    }
    let output = output.unwrap();
    let output_str = String::from_utf8_lossy(&output.stdout);
    let v: Value = serde_json::from_str(&output_str).unwrap_or(Value::Null);
    if let Some(prefsrc) = v.get(0).and_then(|item| item.get("prefsrc")).and_then(|s| s.as_str()) {
        prefsrc.to_string()
    } else {
        "N/A".to_string()
    }
}

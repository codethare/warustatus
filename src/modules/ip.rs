use std::process::Command;

pub fn print_ip_address() -> String {
    let output = Command::new("ip")
        .args(&["route", "get", "8.8.8.8"])
        .output();

    let Ok(output) = output else {
        return "N/A".to_string();
    };

    if !output.status.success() {
        return "N/A".to_string();
    }

    let output_str = String::from_utf8_lossy(&output.stdout);
    
    // 使用字符串处理代替JSON解析
    output_str
        .split_whitespace()
        .skip_while(|&s| s != "src")
        .nth(1)
        .map(|s| s.to_string())
        .unwrap_or_else(|| "N/A".to_string())
}

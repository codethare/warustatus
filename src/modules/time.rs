use chrono::Local;

pub fn current_time() -> String {
    Local::now().format("%H:%M:%S").to_string()
}

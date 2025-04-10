use chrono::Local;

pub fn current_time() -> String {
    Local::now().format("%H:%M").to_string()
}

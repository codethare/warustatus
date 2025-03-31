use chrono::Local;

pub fn print_date() -> String {
    let now = Local::now();
    now.format("%a %m-%d %T").to_string()
}


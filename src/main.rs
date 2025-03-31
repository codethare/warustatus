mod network;
mod ip;
mod memory;
mod battery;
mod date;
mod cpu;


use std::thread;
use std::time::Duration;

fn main() {
    let mut prev_cpu = cpu::read_proc_stat_sync().expect("无法读取 /proc/stat");

    loop {
        let network_info = network::print_network_speed();
        let ip_info = ip::print_ip_address();
        let mem_info = memory::print_mem();
        let bat_info = battery::print_bat();
        let date_info = date::print_date();
        let cpu_usage = match cpu::print_cpu_usage(&prev_cpu) {
            Some((barchart, new_cpu)) => {
                prev_cpu = new_cpu;
                format!("{}", barchart)
            },
            None => "N/A".to_string()
        };

        let status_parts = vec![
            mem_info,
            network_info,
            ip_info,
            cpu_usage,
            bat_info,
            date_info,
        ];

        println!("{}", status_parts.join(" "));
        thread::sleep(Duration::from_secs(2));
    }
}


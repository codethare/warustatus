mod network;
mod ip;
mod memory;
mod battery;
mod date;

use std::thread;
use std::time::Duration;

fn main() {

    loop {
        let network_info = network::print_network_speed();
        let ip_info = ip::print_ip_address();
        let mem_info = memory::print_mem();
        let bat_info = battery::print_bat();
        let date_info = date::print_date();

        let status_parts = vec![
            mem_info,
            network_info,
            ip_info,
            bat_info,
            date_info,
        ];

        println!("{}", status_parts.join(" "));
        thread::sleep(Duration::from_secs(2));
    }
}


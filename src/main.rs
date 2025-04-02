mod modules;

use modules::{
    battery::BatteryInfo,
    cpu::{CpuLoad, CpuTemp},
    memory::MemoryInfo,
    network::{get_ip, NetworkStats},
    time::current_time,
};
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut net_stats = NetworkStats::new();
    let mut cpu_load = CpuLoad::new()?;

    loop {
        // 收集指标数据
        net_stats.update();
        let cpu_usage = cpu_load.update()?;
        let mem = MemoryInfo::now();
        let cpu_temp = CpuTemp::now();
        let bat = BatteryInfo::now();

        // 格式化输出
        println!(
            "{:.1} ▲{:.1} ▼{:.1} {} {:.0}% {:.1}°C {} {}",
            mem.available_gb(),
            net_stats.tx_mbps,
            net_stats.rx_mbps,
            get_ip(),
            cpu_usage,
            cpu_temp.celsius,
            bat,
            current_time()
        );

        std::thread::sleep(Duration::from_secs(2));
    }
}

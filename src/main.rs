mod modules;

use modules::{
    battery::BatteryInfo,
    cpu::{CpuLoad, CpuTemp},
    memory::MemoryInfo,
    network::NetworkStats,
    ip::print_ip_address, // 重命名避免冲突
    time::current_time as get_current_time, // 重命名避免冲突
};
use std::time::Duration;
use tokio::sync::watch; // 引入 watch channel
use tokio::time::interval;

// 使用 Tokio 的 main 宏
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // --- 创建 Watch Channels ---
    // 用于在任务间共享最新数据
    // tx 是发送端, rx 是接收端
    let (bat_tx, bat_rx) = watch::channel(BatteryInfo::default());
    let (cpu_load_tx, cpu_load_rx) = watch::channel(0.0f64); // CPU 负载是 f64
    let (mem_tx, mem_rx) = watch::channel(MemoryInfo::default());
    let (cpu_temp_tx, cpu_temp_rx) = watch::channel(CpuTemp::default());
    let (ip_tx, ip_rx) = watch::channel("N/A".to_string()); // IP 地址是 String
    let (net_tx, net_rx) = watch::channel((0.0f64, 0.0f64)); // 网络速率 (tx, rx) 是 f64 元组
    let (time_tx, time_rx) = watch::channel(get_current_time()); // 时间是 String

    // --- 初始化需要状态的结构体 ---
    // CpuLoad 需要在循环外创建和维护状态
    let mut cpu_load_monitor = CpuLoad::new()?;
    // NetworkStats 也需要维护状态
    let mut net_stats_monitor = NetworkStats::new();

    // --- 启动各个监控任务 ---

    // 电池任务 (60 分钟)
    tokio::spawn(async move {
        let mut interval = interval(Duration::from_secs(60 * 60));
        // 立即执行一次，避免启动时没有数据
        let initial_bat_info = BatteryInfo::now();
        let _ = bat_tx.send(initial_bat_info);
        loop {
            interval.tick().await;
            // 使用 tokio::task::spawn_blocking 来运行同步代码
            let bat_info = tokio::task::spawn_blocking(BatteryInfo::now).await.unwrap_or_default();
            // 发送数据，忽略错误（如果接收端已关闭）
            let _ = bat_tx.send(bat_info);
        }
    });


    // CPU 负载任务 (10 秒)
    tokio::spawn(async move {
        let mut interval = interval(Duration::from_secs(10));
         // 立即执行一次
        if let Ok(initial_load) = cpu_load_monitor.update() {
             let _ = cpu_load_tx.send(initial_load);
        }
        loop {
            interval.tick().await;
            // update 本身是 CPU 密集型且访问 /proc，不算严格的 IO 阻塞，但可以考虑 spawn_blocking
            match cpu_load_monitor.update() {
                 Ok(load) => { let _ = cpu_load_tx.send(load); },
                 Err(e) => eprintln!("Error updating CPU load: {}", e), // 记录错误
            }
        }
    });


    // 内存任务 (10 秒)
    tokio::spawn(async move {
        let mut interval = interval(Duration::from_secs(10));
         // 立即执行一次
        let initial_mem_info = tokio::task::spawn_blocking(MemoryInfo::now).await.unwrap_or_default();
        let _ = mem_tx.send(initial_mem_info);
        loop {
            interval.tick().await;
             let mem_info = tokio::task::spawn_blocking(MemoryInfo::now).await.unwrap_or_default();
            let _ = mem_tx.send(mem_info);
        }
    });

    // CPU 温度任务 (30 秒)
    tokio::spawn(async move {
        let mut interval = interval(Duration::from_secs(30));
         // 立即执行一次
        let initial_temp_info = tokio::task::spawn_blocking(CpuTemp::now).await.unwrap_or_default();
        let _ = cpu_temp_tx.send(initial_temp_info);
        loop {
            interval.tick().await;
             let temp_info = tokio::task::spawn_blocking(CpuTemp::now).await.unwrap_or_default();
            let _ = cpu_temp_tx.send(temp_info);
        }
    });

    // IP 地址任务 (60 秒)
    tokio::spawn(async move {
        let mut interval = interval(Duration::from_secs(60));
         // 立即执行一次
        let initial_ip = tokio::task::spawn_blocking(print_ip_address).await.unwrap_or_else(|_| "N/A".to_string());
        let _ = ip_tx.send(initial_ip);
        loop {
            interval.tick().await;
            // Command::new 是阻塞的，需要 spawn_blocking
            let ip = tokio::task::spawn_blocking(print_ip_address)
                .await
                .unwrap_or_else(|_| "N/A".to_string()); // 处理 spawn_blocking 可能的错误
            let _ = ip_tx.send(ip);
        }
    });


    // 网络流量任务 (2 秒)
    tokio::spawn(async move {
        let mut interval = interval(Duration::from_secs(2));
         // 立即执行一次 (但速率会不准，先获取基线)
        net_stats_monitor.update(); // 获取初始读数，但不发送
        tokio::time::sleep(Duration::from_secs(2)).await; // 等待第一个有效间隔
        net_stats_monitor.update(); // 计算第一个有效速率
        let _ = net_tx.send((net_stats_monitor.tx_mbps, net_stats_monitor.rx_mbps));

        loop {
            interval.tick().await;
            // update 内部读取 /sys，可以考虑 spawn_blocking，但频率较高可能影响不大
            net_stats_monitor.update();
            let _ = net_tx.send((net_stats_monitor.tx_mbps, net_stats_monitor.rx_mbps));
        }
    });

    // 时间任务 (60 秒) - 这个其实没必要单独任务，可以在打印循环里做，但按要求实现
    tokio::spawn(async move {
        let mut interval = interval(Duration::from_secs(60));
         // 立即执行一次 (已在 channel 初始化时完成)
        loop {
            interval.tick().await;
            let time_str = get_current_time();
            let _ = time_tx.send(time_str);
        }
    });

    // --- 主打印循环 ---
    // 控制打印频率，例如每秒打印一次
    let mut print_interval = interval(Duration::from_secs(1));
    loop {
        print_interval.tick().await; // 等待打印间隔

        // 从 watch channel 获取最新数据 (使用 borrow 获取引用，避免克隆开销)
        let bat_val = bat_rx.borrow();
        let cpu_load_val = *cpu_load_rx.borrow(); // f64 可以直接解引用复制
        let mem_val = mem_rx.borrow();
        let cpu_temp_val = cpu_temp_rx.borrow();
        let ip_val = ip_rx.borrow();
        let net_val = *net_rx.borrow(); // 元组可以解引用复制
        let time_val = time_rx.borrow(); // 在打印前获取最新时间可能更好，但按原逻辑

        // 格式化输出
        let temp_display = if cpu_temp_val.celsius < 0.0 {
            "N/A".to_string() // 如果温度是标记值，显示 N/A
        } else {
            format!("{:.1}°C", cpu_temp_val.celsius)
        };

        println!(
            "{:.1}  -{:.1} +{:.1}  {}  {:.0}%  {}  {}  {}",
            mem_val.available_mb(), // 使用 GB 输出
            net_val.0, // tx_mbps
            net_val.1, // rx_mbps
            *ip_val,
            cpu_load_val,
            temp_display, // 使用处理过的温度字符串
            *bat_val,
            *time_val
        );
    }
    // 注意：由于上面的 loop 无限循环，Ok(()) 永远不会被返回，除非程序被外部中断。
    // Ok(())
}

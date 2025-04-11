use std::{
    collections::HashMap,
    time::{Duration, Instant},
    sync::Arc,
};
use tokio::{sync::{watch, Notify}, time::interval};

mod modules;
use modules::{
    battery::BatteryInfo,
    cpu::{CpuLoad, CpuTemp},
    ip::print_ip_address,
    memory::MemoryInfo,
    network::NetworkStats,
    time::current_time as get_current_time,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 所有 watch channel
    let (bat_tx, bat_rx) = watch::channel(BatteryInfo::default());
    let (cpu_load_tx, cpu_load_rx) = watch::channel(0.0);
    let (mem_tx, mem_rx) = watch::channel(MemoryInfo::default());
    let (cpu_temp_tx, cpu_temp_rx) = watch::channel(CpuTemp::default());
    let (ip_tx, ip_rx) = watch::channel("N/A".into());
    let (net_tx, net_rx) = watch::channel((0.0, 0.0));
    let (time_tx, time_rx) = watch::channel(get_current_time());

    let notify = Arc::new(Notify::new());
    let print_notify = notify.clone();

    // 打印器任务
    tokio::spawn(async move {
        loop {
            print_notify.notified().await;

            let bat = bat_rx.borrow(); // 引用（BatteryInfo 非 Copy）
            let cpu = *cpu_load_rx.borrow(); // f64 是 Copy，直接解引用
            let mem = mem_rx.borrow();
            let temp = cpu_temp_rx.borrow();
            let ip = ip_rx.borrow();
            let net = *net_rx.borrow(); // tuple (f64, f64) 是 Copy
            let time = time_rx.borrow();

            let temp_str = if temp.celsius < 0.0 {
                "N/A".into()
            } else {
                format!("{:.1}°C", temp.celsius)
            };

            println!(
                "{:.1} -{:.1} +{:.1} {} {:.0}% {} {} {}",
                mem.available_mb(),
                net.0,
                net.1,
                *ip,
                cpu,
                temp_str,
                *bat,
                *time
            );
        }

    });

    // 启动调度器任务
    Scheduler::new(
        bat_tx,
        cpu_load_tx,
        mem_tx,
        cpu_temp_tx,
        ip_tx,
        net_tx,
        time_tx,
        notify,
    )
    .run()
    .await;

    Ok(())
}

struct Scheduler {
    last_run: HashMap<&'static str, Instant>,
    notify: Arc<Notify>,
    bat_tx: watch::Sender<BatteryInfo>,
    cpu_load_tx: watch::Sender<f64>,
    mem_tx: watch::Sender<MemoryInfo>,
    cpu_temp_tx: watch::Sender<CpuTemp>,
    ip_tx: watch::Sender<String>,
    net_tx: watch::Sender<(f64, f64)>,
    time_tx: watch::Sender<String>,
    cpu_monitor: CpuLoad,
    net_monitor: NetworkStats,
}

impl Scheduler {
    fn new(
        bat_tx: watch::Sender<BatteryInfo>,
        cpu_load_tx: watch::Sender<f64>,
        mem_tx: watch::Sender<MemoryInfo>,
        cpu_temp_tx: watch::Sender<CpuTemp>,
        ip_tx: watch::Sender<String>,
        net_tx: watch::Sender<(f64, f64)>,
        time_tx: watch::Sender<String>,
        notify: Arc<Notify>,
    ) -> Self {
        Self {
            last_run: HashMap::new(),
            notify,
            bat_tx,
            cpu_load_tx,
            mem_tx,
            cpu_temp_tx,
            ip_tx,
            net_tx,
            time_tx,
            cpu_monitor: CpuLoad::new().unwrap(),
            net_monitor: NetworkStats::new(),
        }
    }

    async fn run(mut self) {
        let mut ticker = interval(Duration::from_millis(500));
        loop {
            ticker.tick().await;
            let now = Instant::now();

            // 任务调度区
            if self.should_run("bat", now, 60) {
                if let Ok(data) = tokio::task::spawn_blocking(BatteryInfo::now).await {
                    let _ = self.bat_tx.send(data);
                    self.notify.notify_one();
                }
            }

            if self.should_run("cpu", now, 10) {
                if let Ok(val) = self.cpu_monitor.update() {
                    let _ = self.cpu_load_tx.send(val);
                    self.notify.notify_one();
                }
            }

            if self.should_run("mem", now, 10) {
                if let Ok(data) = tokio::task::spawn_blocking(MemoryInfo::now).await {
                    let _ = self.mem_tx.send(data);
                    self.notify.notify_one();
                }
            }

            if self.should_run("temp", now, 30) {
                if let Ok(data) = tokio::task::spawn_blocking(CpuTemp::now).await {
                    let _ = self.cpu_temp_tx.send(data);
                    self.notify.notify_one();
                }
            }

            if self.should_run("ip", now, 60) {
                if let Ok(data) = tokio::task::spawn_blocking(print_ip_address).await {
                    let _ = self.ip_tx.send(data);
                    self.notify.notify_one();
                }
            }

            if self.should_run("net", now, 2) {
                self.net_monitor.update();
                let _ = self.net_tx.send((self.net_monitor.tx_mbps, self.net_monitor.rx_mbps));
                self.notify.notify_one();
            }

            if self.should_run("time", now, 60) {
                let _ = self.time_tx.send(get_current_time());
                self.notify.notify_one();
            }
        }
    }

    fn should_run(&mut self, key: &'static str, now: Instant, sec: u64) -> bool {
        let run = self
            .last_run
            .get(key)
            .map_or(true, |&t| now.duration_since(t) >= Duration::from_secs(sec));
        if run {
            self.last_run.insert(key, now);
        }
        run
    }
}


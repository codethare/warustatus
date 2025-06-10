use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{
    sync::{watch, Notify},
    time::interval,
};

mod modules;
use modules::{
    battery::BatteryInfo,
    cpu::{CpuLoad, CpuTemp},
    memory::MemoryInfo,
    network::NetworkStats,
    time::current_time as get_current_time,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 所有 watch channel
    let (bat_tx, mut bat_rx) = watch::channel(BatteryInfo::default());
    let (cpu_load_tx, mut cpu_load_rx) = watch::channel(0.0);
    let (mem_tx, mut mem_rx) = watch::channel(MemoryInfo::default());
    let (cpu_temp_tx, mut cpu_temp_rx) = watch::channel(CpuTemp::default());
    let (net_tx, mut net_rx) = watch::channel((0.0, 0.0));
    let (time_tx, mut time_rx) = watch::channel(get_current_time());

    let notify = Arc::new(Notify::new());
    let print_notify = notify.clone();

    // 打印器任务
    tokio::spawn(async move {
        loop {
            print_notify.notified().await;

            // 使用 a guard 来确保锁在作用域结束时被释放
            let bat_guard = bat_rx.borrow_and_update();
            let cpu_guard = cpu_load_rx.borrow_and_update();
            let mem_guard = mem_rx.borrow_and_update();
            let temp_guard = cpu_temp_rx.borrow_and_update();
            let net_guard = net_rx.borrow_and_update();
            let time_guard = time_rx.borrow_and_update();
            
            let bat = &*bat_guard;
            let cpu = *cpu_guard; // f64 是 Copy，直接解引用
            let mem = &*mem_guard;
            let temp = &*temp_guard;
            let net = *net_guard; // tuple (f64, f64) 是 Copy
            let time = &*time_guard;


            let temp_str = if temp.celsius < 0.0 {
                "N/A".to_string()
            } else {
                format!("{:.1}°C", temp.celsius)
            };

            println!(
                "Mem:{:.1}G | Net:-{:.1}M/+{:.1}M | CPU:{:.1}% {} | {} | {}",
                mem.available_mb(),
                net.0,
                net.1,
                cpu,
                temp_str,
                bat,
                time
            );
        }
    });

    // 启动调度器任务
    let mut scheduler = Scheduler::new(
        bat_tx,
        cpu_load_tx,
        mem_tx,
        cpu_temp_tx,
        net_tx,
        time_tx,
        notify,
    )?;
    scheduler.run().await;

    Ok(())
}

struct Scheduler {
    last_run: HashMap<&'static str, Instant>,
    notify: Arc<Notify>,
    bat_tx: watch::Sender<BatteryInfo>,
    cpu_load_tx: watch::Sender<f64>,
    mem_tx: watch::Sender<MemoryInfo>,
    cpu_temp_tx: watch::Sender<CpuTemp>,
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
        net_tx: watch::Sender<(f64, f64)>,
        time_tx: watch::Sender<String>,
        notify: Arc<Notify>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            last_run: HashMap::new(),
            notify,
            bat_tx,
            cpu_load_tx,
            mem_tx,
            cpu_temp_tx,
            net_tx,
            time_tx,
            cpu_monitor: CpuLoad::new()?,
            net_monitor: NetworkStats::new(),
        })
    }

    async fn run(&mut self) {
        let mut ticker = interval(Duration::from_millis(500));
        loop {
            ticker.tick().await;
            let now = Instant::now();

            // --- 任务调度区 ---

            // 电池信息 (每 60 秒)
            if self.should_run("bat", now, 60) {
                let tx = self.bat_tx.clone();
                let notify = self.notify.clone();
                tokio::spawn(async move {
                    if let Ok(Ok(data)) = tokio::task::spawn_blocking(BatteryInfo::now).await {
                        if tx.send(data).is_ok() {
                            notify.notify_one();
                        }
                    }
                });
            }

            // CPU 负载 (每 10 秒)
            if self.should_run("cpu", now, 10) {
                let tx = self.cpu_load_tx.clone();
                let notify = self.notify.clone();
                // `update` 是同步的，可能会阻塞，所以使用 spawn_blocking
                // 我们需要克隆 monitor 或者使用 Arc<Mutex<>>
                let mut cpu_monitor = self.cpu_monitor.clone();
                tokio::spawn(async move {
                    if let Ok(Ok(val)) = tokio::task::spawn_blocking(move || cpu_monitor.update()).await {
                        if tx.send(val).is_ok() {
                           notify.notify_one();
                        }
                    }
                });
            }

            // 内存信息 (每 10 秒)
            if self.should_run("mem", now, 10) {
                let tx = self.mem_tx.clone();
                let notify = self.notify.clone();
                tokio::spawn(async move {
                    if let Ok(Ok(data)) = tokio::task::spawn_blocking(MemoryInfo::now).await {
                       if tx.send(data).is_ok() {
                           notify.notify_one();
                       }
                    }
                });
            }

            // CPU 温度 (每 30 秒)
            if self.should_run("temp", now, 30) {
                let tx = self.cpu_temp_tx.clone();
                let notify = self.notify.clone();
                tokio::spawn(async move {
                    if let Ok(Ok(data)) = tokio::task::spawn_blocking(CpuTemp::now).await {
                        if tx.send(data).is_ok() {
                            notify.notify_one();
                        }
                    }
                });
            }

            // 网络速度 (每 2 秒)
            if self.should_run("net", now, 2) {
                 let tx = self.net_tx.clone();
                 let notify = self.notify.clone();
                 let mut net_monitor = self.net_monitor.clone();
                 tokio::spawn(async move {
                     if let Ok(Ok(_)) = tokio::task::spawn_blocking(move || net_monitor.update()).await {
                        if tx.send((net_monitor.tx_mbps, net_monitor.rx_mbps)).is_ok() {
                            notify.notify_one();
                        }
                     }
                 });
            }

            // 当前时间 (每 60 秒)
            if self.should_run("time", now, 60) {
                if self.time_tx.send(get_current_time()).is_ok() {
                    self.notify.notify_one();
                }
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

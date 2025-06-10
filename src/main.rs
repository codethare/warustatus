use std::{
    collections::HashMap,
    error::Error,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{
    sync::{watch, Notify},
    time::interval,
};

// --- 模块导入 ---
// 确保这些模块中的结构体都实现了所需的 trait (如 Clone, Default)
mod modules;
use modules::{
    battery::BatteryInfo,
    cpu::{CpuLoad, CpuTemp},
    memory::MemoryInfo,
    network::NetworkStats,
    time::current_time as get_current_time,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // ---- 1. 初始化 Channels 和 Notifier ----
    // `watch` 用于广播状态，`Notify` 用于触发更新
    let (bat_tx, mut bat_rx) = watch::channel(BatteryInfo::default());
    let (cpu_load_tx, mut cpu_load_rx) = watch::channel(0.0);
    let (mem_tx, mut mem_rx) = watch::channel(MemoryInfo::default());
    let (cpu_temp_tx, mut cpu_temp_rx) = watch::channel(CpuTemp::default());
    let (net_tx, mut net_rx) = watch::channel((0.0, 0.0)); // (rx, tx)
    let (time_tx, mut time_rx) = watch::channel(get_current_time());

    let notify = Arc::new(Notify::new());

    // ---- 2. 打印任务 ----
    // 派生一个独立的异步任务，专门负责打印信息
    let print_notify = notify.clone();
    tokio::spawn(async move {
        loop {
            // 等待调度器发来的通知，表示有新数据了
            print_notify.notified().await;

            let bat = bat_rx.borrow();
            let cpu = *cpu_load_rx.borrow();
            let mem = mem_rx.borrow();
            let temp = cpu_temp_rx.borrow();
            let net = *net_rx.borrow();
            let time = time_rx.borrow();

            let temp_str = if temp.celsius < 0.0 {
                "N/A".to_string()
            } else {
                format!("{:.1}°C", temp.celsius)
            };
            
            // 优化了打印格式，使其更紧凑和易读
            println!(
                "Mem: {:.1}G | Net: ↓{:.1}M/s ↑{:.1}M/s | CPU: {:.1}% {} | {} | {}",
                mem.available_mb(),
                net.0, // 下载速度
                net.1, // 上传速度
                cpu,
                temp_str,
                *bat,
                *time
            );
        }
    });

    // ---- 3. 调度器任务 ----
    // 创建 Scheduler 并移交所有权给 `run` 方法
    let scheduler = Scheduler::new(
        bat_tx,
        cpu_load_tx,
        mem_tx,
        cpu_temp_tx,
        net_tx,
        time_tx,
        notify,
    )?;

    // 调用 run 会消耗 scheduler，因为它拥有了 self
    scheduler.run().await;

    Ok(())
}

// Scheduler 负责调度所有数据采集任务
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
    // 构造函数
    fn new(
        bat_tx: watch::Sender<BatteryInfo>,
        cpu_load_tx: watch::Sender<f64>,
        mem_tx: watch::Sender<MemoryInfo>,
        cpu_temp_tx: watch::Sender<CpuTemp>,
        net_tx: watch::Sender<(f64, f64)>,
        time_tx: watch::Sender<String>,
        notify: Arc<Notify>,
    ) -> Result<Self, Box<dyn Error>> {
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

    // 主运行循环
    // 🔥 **核心修正**: `run` 方法获得 `self` 的所有权 (`mut self` 而不是 `&mut self`)
    // 这解决了生命周期问题，因为 `self` 现在和 `run` 方法共存亡
    async fn run(mut self) {
        let mut ticker = interval(Duration::from_millis(500));

        loop {
            ticker.tick().await;
            let now = Instant::now();

            // -- 任务调度逻辑 --

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

            if self.should_run("cpu", now, 10) {
                let tx = self.cpu_load_tx.clone();
                let notify = self.notify.clone();
                let mut monitor = self.cpu_monitor.clone(); // 克隆 monitor 以移入新线程
                tokio::spawn(async move {
                    if let Ok(Ok(val)) = tokio::task::spawn_blocking(move || monitor.update()).await {
                        if tx.send(val).is_ok() {
                            notify.notify_one();
                        }
                    }
                });
            }

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
            
            if self.should_run("net", now, 2) {
                let tx = self.net_tx.clone();
                let notify = self.notify.clone();
                let mut monitor = self.net_monitor.clone();
                tokio::spawn(async move {
                    if tokio::task::spawn_blocking(move || monitor.update()).await.is_ok() {
                       if tx.send((monitor.rx_mbps, monitor.tx_mbps)).is_ok() {
                            notify.notify_one();
                        }
                    }
                });
            }
        }
    }

    // 判断任务是否应该运行
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

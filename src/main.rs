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
    // ---- 初始化 channels ----
    // 使用 watch channel 来广播系统状态的更新
    // `tx` 是发送端, `rx` 是接收端
    let (bat_tx, mut bat_rx) = watch::channel(BatteryInfo::default());
    let (cpu_load_tx, mut cpu_load_rx) = watch::channel(0.0);
    let (mem_tx, mut mem_rx) = watch::channel(MemoryInfo::default());
    let (cpu_temp_tx, mut cpu_temp_rx) = watch::channel(CpuTemp::default());
    let (net_tx, mut net_rx) = watch::channel((0.0, 0.0));
    let (time_tx, mut time_rx) = watch::channel(get_current_time());

    // 使用 Notify 来通知打印任务有新的数据需要显示
    let notify = Arc::new(Notify::new());
    let print_notify = notify.clone();

    // ---- 打印任务 ----
    // 这个任务会一直等待通知，然后打印所有模块的最新状态
    tokio::spawn(async move {
        loop {
            // 等待来自调度器的通知
            print_notify.notified().await;

            // 使用 `borrow` 获取 channel 中的最新值。
            // `Ref` (guard) 会在作用域结束时自动释放锁
            let bat = bat_rx.borrow();
            let cpu = *cpu_load_rx.borrow();
            let mem = mem_rx.borrow();
            let temp = cpu_temp_rx.borrow();
            let net = *net_rx.borrow();
            let time = time_rx.borrow();

            // 格式化 CPU 温度，如果无效则显示 "N/A"
            let temp_str = if temp.celsius < 0.0 {
                "N/A".to_string()
            } else {
                format!("{:.1}°C", temp.celsius)
            };
            
            // 优化了打印格式，使其更易读
            println!(
                "Mem: {:.1}G | Net: ↓{:.1}M ↑{:.1}M | CPU: {:.1}% {} | {} | {}",
                mem.available_mb(),
                net.0, // rx
                net.1, // tx
                cpu,
                temp_str,
                *bat, // BatteryInfo 实现了 Display trait
                *time
            );
        }
    });

    // ---- 调度器任务 ----
    // 创建并运行调度器，它会定期更新所有系统信息
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

// Scheduler 负责管理和调度所有信息收集任务
struct Scheduler {
    last_run: HashMap<&'static str, Instant>,
    notify: Arc<Notify>,
    bat_tx: watch::Sender<BatteryInfo>,
    cpu_load_tx: watch::Sender<f64>,
    mem_tx: watch::Sender<MemoryInfo>,
    cpu_temp_tx: watch::Sender<CpuTemp>,
    net_tx: watch::Sender<(f64, f64)>,
    time_tx: watch::Sender<String>,
    // 为了在异步任务中使用，这些需要能被克隆
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
            cpu_monitor: CpuLoad::new()?, // new() 可能返回错误，所以用 `?` 传播错误
            net_monitor: NetworkStats::new(),
        })
    }

    // 主运行循环
    async fn run(&mut self) {
        // 每 500ms 触发一次循环
        let mut ticker = interval(Duration::from_millis(500));

        loop {
            ticker.tick().await;
            let now = Instant::now();

            // -- 任务调度 --
            // 每个任务都使用 `tokio::spawn` 成为独立的异步任务，不会互相阻塞

            if self.should_run("bat", now, 60) {
                let tx = self.bat_tx.clone();
                let notify = self.notify.clone();
                tokio::spawn(async move {
                    // `spawn_blocking` 用于执行阻塞的 I/O 操作
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
            
            if self.should_run("net", now, 2) {
                let tx = self.net_tx.clone();
                let notify = self.notify.clone();
                let mut monitor = self.net_monitor.clone(); // 克隆
                tokio::spawn(async move {
                    // 假设 `update` 是阻塞的
                    if tokio::task::spawn_blocking(move || monitor.update()).await.is_ok() {
                       if tx.send((monitor.rx_mbps, monitor.tx_mbps)).is_ok() {
                            notify.notify_one();
                        }
                    }
                });
            }

            if self.should_run("time", now, 60) {
                if self.time_tx.send(get_current_time()).is_ok() {
                    self.notify.notify_one();
                }
            }
        }
    }

    // 判断任务是否应该运行
    fn should_run(&mut self, key: &'static str, now: Instant, sec: u64) -> bool {
        let deadline = self.last_run.get(key).map_or(None, |t| Some(*t + Duration::from_secs(sec)));
        
        let run = match deadline {
            None => true, // 如果从未运行过，则运行
            Some(d) => now >= d, // 如果到了运行时间，则运行
        };

        if run {
            self.last_run.insert(key, now);
        }
        run
    }
}

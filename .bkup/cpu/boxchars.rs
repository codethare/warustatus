use std::fs::File;
use std::io::{BufRead, BufReader};

#[derive(Clone, Copy)]
pub struct CpuTime {
    pub idle: u64,
    pub non_idle: u64,
}

impl CpuTime {
    pub fn from_str(s: &str) -> Option<Self> {
        let mut parts = s.split_whitespace();
        let user = parts.next()?.parse::<u64>().ok()?;
        let nice = parts.next()?.parse::<u64>().ok()?;
        let system = parts.next()?.parse::<u64>().ok()?;
        let idle = parts.next()?.parse::<u64>().ok()?;
        let iowait = parts.next()?.parse::<u64>().ok()?;
        let irq = parts.next()?.parse::<u64>().ok()?;
        let softirq = parts.next()?.parse::<u64>().ok()?;
        Some(Self {
            idle: idle + iowait,
            non_idle: user + nice + system + irq + softirq,
        })
    }

    pub fn utilization(&self, old: CpuTime) -> f64 {
        let total = self.idle + self.non_idle;
        let old_total = old.idle + old.non_idle;
        let totald = total.saturating_sub(old_total);
        if totald == 0 {
            0.0
        } else {
            let non_idle_diff = self.non_idle.saturating_sub(old.non_idle);
            (non_idle_diff as f64) / (totald as f64)
        }
    }
}

/// 同步读取 /proc/stat，返回 (总体, 每核列表)
pub fn read_proc_stat_sync() -> Option<(CpuTime, Vec<CpuTime>)> {
    let file = File::open("/proc/stat").ok()?;
    let reader = BufReader::new(file);
    let mut total: Option<CpuTime> = None;
    let mut per_core = Vec::new();
    for line in reader.lines() {
        if let Ok(line) = line {
            let line = line.trim();
            if line.starts_with("cpu ") {
                let s = line.trim_start_matches("cpu ");
                total = CpuTime::from_str(s);
            } else if line.starts_with("cpu") {
                // 如 "cpu0", "cpu1", etc.
                let s = line.splitn(2, ' ').nth(1)?;
                if let Some(core_time) = CpuTime::from_str(s) {
                    per_core.push(core_time);
                }
            }
        }
    }
    total.map(|t| (t, per_core))
}

/// 计算 CPU 利用率并返回 BOXCHARS 条形图（每个字符代表一个逻辑核心）
pub fn print_cpu_usage(prev: &(CpuTime, Vec<CpuTime>)) -> Option<(String, (CpuTime, Vec<CpuTime>))> {
    let new = read_proc_stat_sync()?;
    let mut per_core_utilizations = Vec::new();
    if new.1.len() != prev.1.len() {
        return None;
    }
    for (new_core, old_core) in new.1.iter().zip(prev.1.iter()) {
        per_core_utilizations.push(new_core.utilization(*old_core));
    }
    const BOXCHARS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    let mut barchart = String::new();
    for util in per_core_utilizations {
        let mut idx = (util * 7.0).round() as usize;
        if idx > 7 {
            idx = 7;
        }
        barchart.push(BOXCHARS[idx]);
    }
    Some((barchart, new))
}


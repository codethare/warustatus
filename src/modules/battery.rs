use std::{fmt, fs, io, path::Path};

// 需要 Clone, 因为 Scheduler 中的 monitor 需要被克隆
#[derive(Clone, Debug, Default)]
pub struct BatteryInfo {
    capacity: u8,
    status: String,
}

impl BatteryInfo {
    // 这个函数会进行文件 I/O，是阻塞操作，因此返回 Result
    pub fn now() -> io::Result<Self> {
        let mut capacity = 0;
        let status;

        let bat_path = Path::new("/sys/class/power_supply/BAT0");

        if bat_path.exists() {
            let cap_str = fs::read_to_string(bat_path.join("capacity"))?;
            capacity = cap_str.trim().parse().unwrap_or(0);

            let stat_str = fs::read_to_string(bat_path.join("status"))?;
            status = match stat_str.trim() {
                "Charging" => "".to_string(),    // 充电中
                "Discharging" => "".to_string(), // 放电中
                "Full" => "",        // 已充满
                _ => "battery error".to_string(),            // 未知状态
            };
        } else {
            status = "N/A".to_string(); // 无电池
        }

        Ok(Self { capacity, status })
    }
}

// 实现 Display trait 以便能直接打印
impl fmt::Display for BatteryInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.status == "N/A" {
            write!(f, "{}", self.status)
        } else {
            // 将状态图标和电量百分比结合起来
            write!(f, "{}{}%", self.status, self.capacity)
        }
    }
}

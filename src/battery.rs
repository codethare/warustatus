use std::process::Command;
use tokio::runtime::Runtime;
use zbus::dbus_proxy;
use zbus::Connection;

/// 电池状态枚举
#[derive(Debug)]
pub enum BatteryStatus {
    Charging,
    Discharging,
    Full,
    Unknown,
}

/// 电池信息结构体
#[derive(Debug)]
pub struct BatteryInfo {
    pub percentage: f64,
    pub status: BatteryStatus,
    /// 单位秒（充电时表示 time_to_full，放电时表示 time_to_empty）
    #[allow(dead_code)]
    pub time_remaining: Option<f64>,
}

#[dbus_proxy(interface = "org.freedesktop.UPower")]
trait UPower {
    /// 获取显示设备。对于大部分系统，使用 DisplayDevice 可代表所有电池
    fn get_display_device(&self) -> zbus::Result<zvariant::OwnedObjectPath>;
}

#[dbus_proxy(interface = "org.freedesktop.UPower.Device")]
trait Device {
    /// 当前电量百分比（例如 85.0 表示 85%）
    #[dbus_proxy(property)]
    fn percentage(&self) -> zbus::Result<f64>;
    /// 电池状态代码
    #[dbus_proxy(property)]
    fn state(&self) -> zbus::Result<u32>;
    /// 放电剩余时间，单位秒
    #[dbus_proxy(property)]
    fn time_to_empty(&self) -> zbus::Result<i64>;
    /// 充电剩余时间，单位秒
    #[dbus_proxy(property)]
    fn time_to_full(&self) -> zbus::Result<i64>;
}

/// 异步获取电池信息（仅使用 Upower）
pub async fn get_battery_info_upower() -> Result<BatteryInfo, Box<dyn std::error::Error>> {
    // 建立系统 DBus 连接
    let connection = Connection::system().await?;
    // 创建 UPower 代理
    let upower = UPowerProxy::new(&connection).await?;
    // 获取 DisplayDevice（代表整体电池状态）
    let display_device_path = upower.get_display_device().await?;
    // 创建设备代理，绑定到 DisplayDevice
    let device = DeviceProxy::builder(&connection)
        .path(display_device_path)?
        .build()
        .await?;
    
    // 读取各项属性
    let percentage = device.percentage().await?;
    let state_code = device.state().await?;
    let time_to_empty = device.time_to_empty().await?;
    let time_to_full = device.time_to_full().await?;
    
    // 将状态代码映射为自定义 BatteryStatus（参考 UPower 文档：1=Charging, 2=Discharging, 4=Fully charged）
    let status = match state_code {
        1 => BatteryStatus::Charging,
        2 => BatteryStatus::Discharging,
        4 => BatteryStatus::Full,
        _ => BatteryStatus::Unknown,
    };
    
    // 根据状态选择时间信息
    let time_remaining = match status {
        BatteryStatus::Charging => Some(time_to_full as f64),
        BatteryStatus::Discharging => Some(time_to_empty as f64),
        _ => None,
    };
    
    Ok(BatteryInfo {
        percentage,
        status,
        time_remaining,
    })
}

/// 同步接口：获取电池信息并返回格式化后的字符串
///
/// 如果电池处于放电状态且电量低，则会调用 notify-send 发出通知。
pub fn print_bat() -> String {
    let rt = Runtime::new().unwrap();
    let info = rt.block_on(get_battery_info_upower());
    
    match info {
        Ok(battery_info) => {
            // 电量低时发出通知（放电状态下电量 ≤15% 或 ≤30%）
            if let BatteryStatus::Discharging = battery_info.status {
                if battery_info.percentage <= 15.0 {
                    let _ = Command::new("notify-send")
                        .args(&["-u", "critical", "Battery Warning", &format!("Low battery: {:.0}%", battery_info.percentage)])
                        .output();
                } else if battery_info.percentage <= 30.0 {
                    let _ = Command::new("notify-send")
                        .args(&["-u", "normal", "Battery Warning", &format!("Battery low: {:.0}%", battery_info.percentage)])
                        .output();
                }
            }
            // 如果电池处于 Fully Charged 状态，则显示 "Full"，否则显示百分比
            match battery_info.status {
                BatteryStatus::Full => "Full".to_string(),
                _ => format!("{:.0}%", battery_info.percentage),
            }
        },
        Err(e) => {
            eprintln!("Failed to get battery info: {}", e);
            "N/A".to_string()
        }
    }
}


use serde::Serialize;
use std::process::Command;

#[derive(Debug, Clone, Serialize)]
pub struct BatteryInfo {
    pub percentage: u8,
    pub is_charging: bool,
}

/// Get battery information using pmset command
pub fn get_battery_info() -> Option<BatteryInfo> {
    let output = Command::new("pmset")
        .args(["-g", "batt"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parse output like:
    // Now drawing from 'Battery Power'
    //  -InternalBattery-0 (id=...)	85%; discharging; 3:45 remaining
    // or
    //  -InternalBattery-0 (id=...)	85%; charging; 0:45 until full

    for line in stdout.lines() {
        if line.contains("InternalBattery") {
            // Extract percentage
            if let Some(pct_idx) = line.find('%') {
                // Find the start of the number (work backwards from %)
                let before_pct = &line[..pct_idx];
                let pct_start = before_pct
                    .rfind(|c: char| !c.is_ascii_digit())
                    .map(|i| i + 1)
                    .unwrap_or(0);

                if let Ok(percentage) = before_pct[pct_start..].parse::<u8>() {
                    let is_charging = line.contains("charging") && !line.contains("discharging");
                    return Some(BatteryInfo {
                        percentage,
                        is_charging,
                    });
                }
            }
        }
    }

    None
}

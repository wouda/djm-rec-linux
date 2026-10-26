use djm_common::protocol::HotspotControl;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn};

pub struct HotspotWatchdog {
    mode: String,
    ssid: String,
    password: String,
    running: Arc<AtomicBool>,
}

impl HotspotWatchdog {
    pub fn new(mode: String, ssid: String, password: String, running: Arc<AtomicBool>) -> Self {
        Self {
            mode,
            ssid,
            password,
            running,
        }
    }

    pub async fn run(&self) {
        info!("HotspotWatchdog initialized in mode '{}' (SSID: '{}')", self.mode, self.ssid);

        if self.mode == "off" {
            info!("HotspotWatchdog disabled by configuration (hotspot_mode = 'off').");
            return;
        }

        if self.mode == "always" {
            info!("HotspotWatchdog starting always-on hotspot...");
            if let Err(e) = HotspotControl::start_hotspot(&self.ssid, &self.password) {
                warn!("Failed to start always-on Wi-Fi hotspot: {}", e);
            }
            return;
        }

        // "auto" mode: Wait grace period (15s) then monitor connectivity
        info!("HotspotWatchdog 'auto' mode: waiting 15s startup grace period before monitoring connectivity...");
        for _ in 0..15 {
            if !self.running.load(Ordering::Relaxed) {
                return;
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }

        let mut offline_consecutive_ticks = 0;

        while self.running.load(Ordering::Relaxed) {
            tokio::time::sleep(Duration::from_secs(5)).await;

            if HotspotControl::is_hotspot_active() {
                // Hotspot is active, continue monitoring
                continue;
            }

            if HotspotControl::has_active_network() {
                offline_consecutive_ticks = 0;
            } else {
                offline_consecutive_ticks += 1;
                // If offline for >= 2 consecutive 5s ticks (10s offline after grace period)
                if offline_consecutive_ticks >= 2 {
                    info!("No active Wi-Fi or LAN connection detected. Auto-activating local Wi-Fi Hotspot '{}'...", self.ssid);
                    match HotspotControl::start_hotspot(&self.ssid, &self.password) {
                        Ok(true) => {
                            info!("✓ Local Wi-Fi Hotspot successfully activated at 10.42.0.1:8080 (SSID: '{}')", self.ssid);
                            offline_consecutive_ticks = 0;
                        }
                        Ok(false) => {
                            warn!("Failed to start local Wi-Fi Hotspot via nmcli. Wi-Fi device may be missing or unmanaged.");
                        }
                        Err(e) => {
                            warn!("Error invoking nmcli hotspot: {}", e);
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hotspot_watchdog_creation() {
        let running = Arc::new(AtomicBool::new(true));
        let wd = HotspotWatchdog::new("auto".to_string(), "DJM-Box-Booth".to_string(), "pass1234".to_string(), running);
        assert_eq!(wd.mode, "auto");
        assert_eq!(wd.ssid, "DJM-Box-Booth");
        assert_eq!(wd.password, "pass1234");
    }
}

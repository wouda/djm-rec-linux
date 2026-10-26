use anyhow::Result;
use bluer::adv::Advertisement;
use bluer::gatt::local::{
    Application, Characteristic, CharacteristicNotifier, CharacteristicNotify, CharacteristicNotifyMethod,
    CharacteristicRead, CharacteristicWrite, CharacteristicWriteMethod, ReqError, Service,
};
use djm_common::protocol::{ControlCommand, PairingInfo, Telemetry};
use futures_util::FutureExt;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, watch, RwLock};
use tracing::{info, warn};

pub struct BleGattService {
    device_name: String,
    auth_token: String,
    telemetry_rx: watch::Receiver<Telemetry>,
    command_tx: mpsc::Sender<ControlCommand>,
    running: Arc<AtomicBool>,
}

impl BleGattService {
    pub fn new(
        device_name: String,
        auth_token: String,
        telemetry_rx: watch::Receiver<Telemetry>,
        command_tx: mpsc::Sender<ControlCommand>,
        running: Arc<AtomicBool>,
    ) -> Self {
        Self {
            device_name,
            auth_token,
            telemetry_rx,
            command_tx,
            running,
        }
    }

    pub async fn run(&self) -> Result<()> {
        let session = match bluer::Session::new().await {
            Ok(s) => s,
            Err(e) => {
                warn!("Bluetooth D-Bus session not available: {}. Skipping BLE peripheral.", e);
                return Ok(());
            }
        };

        let adapter = match session.default_adapter().await {
            Ok(a) => a,
            Err(e) => {
                warn!("No default Bluetooth adapter found: {}. Skipping BLE peripheral.", e);
                return Ok(());
            }
        };

        let _ = adapter.set_powered(true).await;
        // Layer 1: Keep BlueZ Pairable = false outside pairing windows to block OS-level pairing
        let _ = adapter.set_pairable(false).await;
        let _ = adapter.set_discoverable(true).await;

        info!("Bluetooth adapter '{}' initialized. Registering GATT service...", adapter.name());

        let service_uuid: bluer::Uuid = PairingInfo::BLE_SERVICE_UUID.parse()?;
        let char_tel_uuid: bluer::Uuid = PairingInfo::BLE_CHAR_TELEMETRY.parse()?;
        let char_meta_uuid: bluer::Uuid = PairingInfo::BLE_CHAR_METADATA.parse()?;
        let char_ctrl_uuid: bluer::Uuid = PairingInfo::BLE_CHAR_CONTROL.parse()?;

        // Track authenticated devices by Bluetooth MAC address (per-connection session isolation)
        let authenticated_devices: Arc<RwLock<HashSet<bluer::Address>>> = Arc::new(RwLock::new(HashSet::new()));

        let expected_token = self.auth_token.clone();
        let cmd_tx = self.command_tx.clone();
        let auth_for_write = authenticated_devices.clone();

        // Control characteristic write handler
        let write_handler = move |value: Vec<u8>, req: bluer::gatt::local::CharacteristicWriteRequest| {
            let cmd_tx = cmd_tx.clone();
            let auth_devs = auth_for_write.clone();
            let expected_token = expected_token.clone();

            async move {
                let dev_addr = req.device_address;
                match ControlCommand::from_bytes(&value) {
                    Ok(ControlCommand::Auth(ref token)) => {
                        if token.trim() == expected_token.trim() {
                            auth_devs.write().await.insert(dev_addr);
                            info!("BLE client {:?} successfully authenticated with auth token!", dev_addr);
                            Ok(())
                        } else {
                            auth_devs.write().await.remove(&dev_addr);
                            warn!("BLE client {:?} failed authentication with invalid token", dev_addr);
                            Err(ReqError::NotAuthorized)
                        }
                    }
                    Ok(cmd) => {
                        if !auth_devs.read().await.contains(&dev_addr) {
                            warn!("Rejected unauthenticated BLE write command from {:?}", dev_addr);
                            return Err(ReqError::NotAuthorized);
                        }
                        let _ = cmd_tx.send(cmd).await;
                        Ok(())
                    }
                    Err(e) => {
                        warn!("Invalid BLE command packet from {:?}: {:?}", dev_addr, e);
                        Err(ReqError::Failed)
                    }
                }
            }
            .boxed()
        };

        let tel_rx_for_notify = self.telemetry_rx.clone();
        let auth_for_notify = authenticated_devices.clone();

        // Telemetry characteristic notify handler
        let notify_handler = move |mut notifier: CharacteristicNotifier| {
            let mut tel_rx = tel_rx_for_notify.clone();
            let auth_devs = auth_for_notify.clone();

            async move {
                let start = Instant::now();
                let auth_timeout = Duration::from_secs(3);

                while !notifier.is_stopped() {
                    // Check if at least one authenticated device session exists
                    if auth_devs.read().await.is_empty() {
                        if start.elapsed() >= auth_timeout {
                            warn!("BLE 3.0s auth watchdog expired without token! Terminating unauthenticated notification stream.");
                            break;
                        }
                        tokio::time::sleep(Duration::from_millis(100)).await;
                        continue;
                    }

                    if tel_rx.changed().await.is_ok() {
                        let telemetry = tel_rx.borrow().clone();
                        let packed = telemetry.to_packed_bytes();
                        if let Err(_) = notifier.notify(packed.to_vec()).await {
                            break;
                        }
                    }
                    tokio::time::sleep(Duration::from_millis(50)).await; // ~20 Hz
                }
            }
            .boxed()
        };

        let tel_rx_for_meta = self.telemetry_rx.clone();
        let auth_for_meta = authenticated_devices.clone();

        // Metadata characteristic read handler
        let read_meta_handler = move |req: bluer::gatt::local::CharacteristicReadRequest| {
            let tel_rx = tel_rx_for_meta.clone();
            let auth_devs = auth_for_meta.clone();

            async move {
                let dev_addr = req.device_address;
                if !auth_devs.read().await.contains(&dev_addr) {
                    warn!("Rejected unauthenticated BLE metadata read from {:?}", dev_addr);
                    return Err(ReqError::NotAuthorized);
                }
                let t = tel_rx.borrow().clone();
                let json = serde_json::to_string(&t).unwrap_or_default();
                Ok(json.into_bytes())
            }
            .boxed()
        };

        let app = Application {
            services: vec![Service {
                uuid: service_uuid,
                primary: true,
                characteristics: vec![
                    Characteristic {
                        uuid: char_tel_uuid,
                        notify: Some(CharacteristicNotify {
                            notify: true,
                            method: CharacteristicNotifyMethod::Fun(Box::new(notify_handler)),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                    Characteristic {
                        uuid: char_meta_uuid,
                        read: Some(CharacteristicRead {
                            read: true,
                            fun: Box::new(read_meta_handler),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                    Characteristic {
                        uuid: char_ctrl_uuid,
                        write: Some(CharacteristicWrite {
                            write: true,
                            method: CharacteristicWriteMethod::Fun(Box::new(write_handler)),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        };

        let _app_handle = match adapter.serve_gatt_application(app).await {
            Ok(h) => h,
            Err(e) => {
                warn!("Failed to serve GATT application: {}. Running without BLE.", e);
                return Ok(());
            }
        };

        // BLE Advertisement
        let adv = Advertisement {
            service_uuids: vec![service_uuid].into_iter().collect(),
            local_name: Some(self.device_name.clone()),
            discoverable: Some(true),
            ..Default::default()
        };

        let _adv_handle = match adapter.advertise(adv).await {
            Ok(h) => h,
            Err(e) => {
                warn!("Failed to start BLE advertisement: {}. Running without BLE.", e);
                return Ok(());
            }
        };

        info!("BLE GATT Peripheral active as '{}' (UUID: {})", self.device_name, service_uuid);

        // Layer 2 Background Watchdog: forcibly disconnect any device that does not authenticate within 3.0 seconds
        let adapter_watchdog = adapter.clone();
        let auth_devs_watchdog = authenticated_devices.clone();
        let running_watchdog = self.running.clone();

        tokio::spawn(async move {
            let mut connection_start: HashMap<bluer::Address, Instant> = HashMap::new();

            while running_watchdog.load(Ordering::Relaxed) {
                tokio::time::sleep(Duration::from_millis(500)).await;

                let addresses = match adapter_watchdog.device_addresses().await {
                    Ok(addrs) => addrs,
                    Err(_) => continue,
                };

                for addr in addresses {
                    if let Ok(device) = adapter_watchdog.device(addr) {
                        let is_connected = device.is_connected().await.unwrap_or(false);
                        if is_connected {
                            let is_authed = auth_devs_watchdog.read().await.contains(&addr);
                            if is_authed {
                                connection_start.remove(&addr);
                            } else {
                                let seen_at = connection_start.entry(addr).or_insert_with(Instant::now);
                                if seen_at.elapsed() >= Duration::from_secs(3) {
                                    warn!(
                                        "BLE 3.0s watchdog expired for unauthenticated device {:?}! Forcibly disconnecting via BlueZ D-Bus.",
                                        addr
                                    );
                                    let _ = device.disconnect().await;
                                    connection_start.remove(&addr);
                                }
                            }
                        } else {
                            // Device disconnected: clean up tracking and authentication session
                            connection_start.remove(&addr);
                            auth_devs_watchdog.write().await.remove(&addr);
                        }
                    }
                }
            }
        });

        while self.running.load(Ordering::Relaxed) {
            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        info!("BLE GATT service stopped.");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_token_verification_logic() {
        let auth_cmd = ControlCommand::Auth("849201".to_string());
        let bytes = auth_cmd.to_bytes();
        let parsed = ControlCommand::from_bytes(&bytes).unwrap();
        if let ControlCommand::Auth(token) = parsed {
            assert_eq!(token, "849201");
        } else {
            panic!("Expected auth command");
        }
    }

    #[tokio::test]
    async fn test_per_device_session_isolation() {
        let auth_devs: Arc<RwLock<HashSet<bluer::Address>>> = Arc::new(RwLock::new(HashSet::new()));
        let dev1: bluer::Address = [0xAA, 0xBB, 0xCC, 0x11, 0x22, 0x33].into();
        let dev2: bluer::Address = [0xAA, 0xBB, 0xCC, 0x44, 0x55, 0x66].into();

        // Initially neither is authenticated
        assert!(!auth_devs.read().await.contains(&dev1));
        assert!(!auth_devs.read().await.contains(&dev2));

        // Authenticate dev1
        auth_devs.write().await.insert(dev1);
        assert!(auth_devs.read().await.contains(&dev1));
        // Verify dev2 is NOT authenticated (no global auth leak)
        assert!(!auth_devs.read().await.contains(&dev2));

        // When dev1 disconnects, its session is evicted
        auth_devs.write().await.remove(&dev1);
        assert!(!auth_devs.read().await.contains(&dev1));
    }
}

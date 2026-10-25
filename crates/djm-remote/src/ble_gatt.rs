use anyhow::Result;
use bluer::adv::Advertisement;
use bluer::gatt::local::{
    Application, Characteristic, CharacteristicNotifier, CharacteristicNotify, CharacteristicNotifyMethod,
    CharacteristicRead, CharacteristicWrite, CharacteristicWriteMethod, ReqError, Service,
};
use djm_common::protocol::{ControlCommand, PairingInfo, Telemetry};
use futures_util::FutureExt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, watch};
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
        let _ = adapter.set_pairable(true).await;
        let _ = adapter.set_discoverable(true).await;

        info!("Bluetooth adapter '{}' initialized. Registering GATT service...", adapter.name());

        let service_uuid: bluer::Uuid = PairingInfo::BLE_SERVICE_UUID.parse()?;
        let char_tel_uuid: bluer::Uuid = PairingInfo::BLE_CHAR_TELEMETRY.parse()?;
        let char_meta_uuid: bluer::Uuid = PairingInfo::BLE_CHAR_METADATA.parse()?;
        let char_ctrl_uuid: bluer::Uuid = PairingInfo::BLE_CHAR_CONTROL.parse()?;

        let is_authenticated = Arc::new(AtomicBool::new(false));
        let expected_token = self.auth_token.clone();
        let cmd_tx = self.command_tx.clone();
        let is_auth_for_write = is_authenticated.clone();

        // Control characteristic write handler
        let write_handler = move |value: Vec<u8>, _req| {
            let cmd_tx = cmd_tx.clone();
            let is_auth = is_auth_for_write.clone();
            let expected_token = expected_token.clone();

            async move {
                match ControlCommand::from_bytes(&value) {
                    Ok(ControlCommand::Auth(ref token)) => {
                        if token.trim() == expected_token.trim() {
                            is_auth.store(true, Ordering::Relaxed);
                            info!("BLE client successfully authenticated with auth token!");
                            Ok(())
                        } else {
                            warn!("BLE client failed authentication with invalid token");
                            Err(ReqError::NotAuthorized)
                        }
                    }
                    Ok(cmd) => {
                        if !is_auth.load(Ordering::Relaxed) {
                            warn!("Rejected unauthenticated BLE write command");
                            return Err(ReqError::NotAuthorized);
                        }
                        let _ = cmd_tx.send(cmd).await;
                        Ok(())
                    }
                    Err(e) => {
                        warn!("Invalid BLE command packet: {:?}", e);
                        Err(ReqError::Failed)
                    }
                }
            }
            .boxed()
        };

        let tel_rx_for_notify = self.telemetry_rx.clone();
        let is_auth_for_notify = is_authenticated.clone();

        // Telemetry characteristic notify handler
        let notify_handler = move |mut notifier: CharacteristicNotifier| {
            let mut tel_rx = tel_rx_for_notify.clone();
            let is_auth = is_auth_for_notify.clone();

            async move {
                // 45-second security watchdog for BLE client authentication
                let start = tokio::time::Instant::now();
                let auth_timeout = Duration::from_secs(45);

                while !notifier.is_stopped() {
                    if !is_auth.load(Ordering::Relaxed) {
                        if start.elapsed() >= auth_timeout {
                            warn!("BLE 45s auth watchdog expired without token! Evicting unauthenticated scanner.");
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
        let is_auth_for_meta = is_authenticated.clone();

        // Metadata characteristic read handler
        let read_meta_handler = move |_req| {
            let tel_rx = tel_rx_for_meta.clone();
            let is_auth = is_auth_for_meta.clone();

            async move {
                if !is_auth.load(Ordering::Relaxed) {
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
}

use crate::protocol::DiscoveredDevice;
use djm_common::prodj::ProDjDevice;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct DeviceInventory {
    pub devices: HashMap<u8, ProDjDevice>,
    pub mixer: Option<ProDjDevice>,
    pub is_six_player_supported: bool,
    pub total_devices_seen: usize,
}

impl Default for DeviceInventory {
    fn default() -> Self {
        Self::new()
    }
}

impl DeviceInventory {
    pub fn new() -> Self {
        Self {
            devices: HashMap::new(),
            mixer: None,
            is_six_player_supported: false,
            total_devices_seen: 0,
        }
    }

    pub fn get_devices(&self) -> Vec<ProDjDevice> {
        let mut result = Vec::new();
        if let Some(m) = &self.mixer {
            result.push(m.clone());
        }
        for dev in self.devices.values() {
            result.push(dev.clone());
        }
        result.sort_by_key(|d| d.player_number);
        result
    }

    /// Updates or inserts a device. Returns `true` if this device was newly discovered or changed.
    pub fn update_device(&mut self, disc: DiscoveredDevice, now_ms: u64) -> bool {
        let is_new = if disc.is_mixer {
            let newly_discovered = self.mixer.is_none();
            let model = disc.device_name.clone();
            // Check for 6-player capability from mixer model
            if model.contains("V10") || model.contains("A9") {
                self.is_six_player_supported = true;
            }

            self.mixer = Some(ProDjDevice {
                player_number: disc.player_number,
                device_name: disc.device_name,
                ip_address: disc.ip_address,
                mac_address: disc.mac_address,
                model,
                firmware: "1.00".to_string(),
                is_mixer: true,
                last_seen_ms: now_ms,
            });
            newly_discovered
        } else {
            let newly_discovered = !self.devices.contains_key(&disc.player_number);
            if disc.player_number == 5 || disc.player_number == 6 {
                self.is_six_player_supported = true;
            }

            self.devices.insert(
                disc.player_number,
                ProDjDevice {
                    player_number: disc.player_number,
                    device_name: disc.device_name.clone(),
                    ip_address: disc.ip_address,
                    mac_address: disc.mac_address,
                    model: disc.device_name,
                    firmware: "1.00".to_string(),
                    is_mixer: false,
                    last_seen_ms: now_ms,
                },
            );
            newly_discovered
        };

        if is_new {
            self.total_devices_seen += 1;
        }

        is_new
    }

    /// Purges devices that haven't sent a keepalive before `cutoff_ms`
    pub fn purge_expired(&mut self, cutoff_ms: u64) -> Vec<u8> {
        let mut expired_numbers = Vec::new();
        self.devices.retain(|&num, dev| {
            if dev.last_seen_ms < cutoff_ms {
                expired_numbers.push(num);
                false
            } else {
                true
            }
        });

        if let Some(ref m) = self.mixer {
            if m.last_seen_ms < cutoff_ms {
                self.mixer = None;
            }
        }

        expired_numbers
    }

    /// Finds the highest available player number based on detected network capabilities
    pub fn highest_free_player_number(&self) -> Option<u8> {
        if self.is_six_player_supported {
            // Check 6, then 5, then 4, 3, 2, 1
            for num in [6, 5, 4, 3, 2, 1] {
                if !self.devices.contains_key(&num) {
                    return Some(num);
                }
            }
        } else {
            // 4-player network: check 4, 3, 2, 1
            for num in [4, 3, 2, 1] {
                if !self.devices.contains_key(&num) {
                    return Some(num);
                }
            }
        }
        None
    }

    /// Returns all occupied player numbers
    pub fn occupied_player_numbers(&self) -> Vec<u8> {
        let mut nums: Vec<u8> = self.devices.keys().copied().collect();
        nums.sort_unstable();
        nums
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn test_device_inventory_player_slot_selection_6_player() {
        let mut inv = DeviceInventory::new();
        // Add DJM-A9 mixer -> marks 6-player supported
        let a9 = DiscoveredDevice {
            device_name: "DJM-A9".to_string(),
            player_number: 0x11,
            ip_address: Ipv4Addr::new(192, 168, 85, 17),
            mac_address: [0; 6],
            is_mixer: true,
        };
        inv.update_device(a9, 1000);
        assert!(inv.is_six_player_supported);

        // Add CDJ-3000 on Player 1 and Player 2
        inv.update_device(DiscoveredDevice {
            device_name: "CDJ-3000".to_string(),
            player_number: 1,
            ip_address: Ipv4Addr::new(192, 168, 85, 1),
            mac_address: [0; 6],
            is_mixer: false,
        }, 1000);
        inv.update_device(DiscoveredDevice {
            device_name: "CDJ-3000".to_string(),
            player_number: 2,
            ip_address: Ipv4Addr::new(192, 168, 85, 2),
            mac_address: [0; 6],
            is_mixer: false,
        }, 1000);

        // Highest free player number on 6-player network should be 6
        assert_eq!(inv.highest_free_player_number(), Some(6));

        // Now if Player 6 is connected, next should be 5
        inv.update_device(DiscoveredDevice {
            device_name: "CDJ-3000".to_string(),
            player_number: 6,
            ip_address: Ipv4Addr::new(192, 168, 85, 6),
            mac_address: [0; 6],
            is_mixer: false,
        }, 1000);
        assert_eq!(inv.highest_free_player_number(), Some(5));
    }

    #[test]
    fn test_device_inventory_player_slot_selection_4_player() {
        let mut inv = DeviceInventory::new();
        // Add DJM-900NXS2 mixer -> 4-player network
        inv.update_device(DiscoveredDevice {
            device_name: "DJM-900NXS2".to_string(),
            player_number: 0x11,
            ip_address: Ipv4Addr::new(192, 168, 85, 17),
            mac_address: [0; 6],
            is_mixer: true,
        }, 1000);
        assert!(!inv.is_six_player_supported);

        // Add CDJ-2000NXS2 on 1, 2, 3
        inv.update_device(DiscoveredDevice {
            device_name: "CDJ-2000NXS2".to_string(),
            player_number: 1,
            ip_address: Ipv4Addr::new(192, 168, 85, 1),
            mac_address: [0; 6],
            is_mixer: false,
        }, 1000);
        inv.update_device(DiscoveredDevice {
            device_name: "CDJ-2000NXS2".to_string(),
            player_number: 2,
            ip_address: Ipv4Addr::new(192, 168, 85, 2),
            mac_address: [0; 6],
            is_mixer: false,
        }, 1000);
        inv.update_device(DiscoveredDevice {
            device_name: "CDJ-2000NXS2".to_string(),
            player_number: 3,
            ip_address: Ipv4Addr::new(192, 168, 85, 3),
            mac_address: [0; 6],
            is_mixer: false,
        }, 1000);

        // On 4-player network, highest free is 4
        assert_eq!(inv.highest_free_player_number(), Some(4));

        // If player 4 is also connected, no slots free
        inv.update_device(DiscoveredDevice {
            device_name: "CDJ-2000NXS2".to_string(),
            player_number: 4,
            ip_address: Ipv4Addr::new(192, 168, 85, 4),
            mac_address: [0; 6],
            is_mixer: false,
        }, 1000);
        assert_eq!(inv.highest_free_player_number(), None);
    }
}

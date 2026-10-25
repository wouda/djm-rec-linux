use alsa::mixer::{MilliBel, Mixer, SelemChannelId, SelemId};
use alsa::Round;
use djm_common::profile::MixerProfile;
use std::ffi::CString;
use thiserror::Error;
use tracing::{info, warn};

#[derive(Error, Debug)]
pub enum MixerError {
    #[error("ALSA error: {0}")]
    Alsa(#[from] alsa::Error),
    #[error("Control element not found: {0}")]
    ControlNotFound(String),
}

pub trait MixerController: Send + Sync {
    fn configure_rec_out(&mut self, profile: &MixerProfile) -> Result<(), MixerError>;
    fn set_capture_pad(&mut self, profile: &MixerProfile, pad_db: i8) -> Result<(), MixerError>;
    fn get_capture_pad(&self, profile: &MixerProfile) -> Result<i8, MixerError>;
}

pub struct AlsaMixerController {
    card_name: String,
}

impl AlsaMixerController {
    pub fn new(card_name: &str) -> Self {
        Self {
            card_name: card_name.to_string(),
        }
    }

    fn open_mixer(&self) -> Result<Mixer, alsa::Error> {
        Mixer::new(&self.card_name, false)
    }
}

impl MixerController for AlsaMixerController {
    fn configure_rec_out(&mut self, profile: &MixerProfile) -> Result<(), MixerError> {
        let mixer = match self.open_mixer() {
            Ok(m) => m,
            Err(e) => {
                warn!("Could not open ALSA mixer for {}: {}", self.card_name, e);
                return Err(MixerError::Alsa(e));
            }
        };

        if let (Some(ctrl_name), Some(ctrl_val)) = (profile.routing_control_name, profile.routing_control_value) {
            let selem_id = SelemId::new(ctrl_name, 0);
            if let Some(selem) = mixer.find_selem(&selem_id) {
                if selem.is_enumerated() {
                    let c_val = CString::new(ctrl_val).unwrap_or_default();
                    if let Ok(count) = selem.get_enum_items() {
                        for idx in 0..count {
                            if let Ok(item_name) = selem.get_enum_item_name(idx) {
                                if item_name.as_bytes() == c_val.as_bytes() {
                                    let _ = selem.set_enum_item(SelemChannelId::mono(), idx);
                                    info!("Configured {} '{}' to index {}", self.card_name, ctrl_name, idx);
                                    break;
                                }
                            }
                        }
                    }
                }
            } else {
                warn!("Routing control element '{}' not found on {}", ctrl_name, self.card_name);
            }
        }

        // Pioneer DJ Mixers: Ensure Microphone routing is active and included in REC OUT
        let mic_controls = [
            "Mic to Rec Out",
            "Mic Output",
            "Microphone to REC OUT",
            "Rec Out Mic",
            "Microphone",
        ];
        for mic_ctrl in &mic_controls {
            let selem_id = SelemId::new(mic_ctrl, 0);
            if let Some(selem) = mixer.find_selem(&selem_id) {
                if selem.has_capture_switch() {
                    let _ = selem.set_capture_switch_all(1);
                    info!("Enabled microphone capture switch '{}' on {}", mic_ctrl, self.card_name);
                } else if selem.is_enumerated() {
                    if let Ok(count) = selem.get_enum_items() {
                        for idx in 0..count {
                            if let Ok(item_name) = selem.get_enum_item_name(idx) {
                                let name_str = item_name.as_str();
                                if name_str.eq_ignore_ascii_case("on")
                                    || name_str.eq_ignore_ascii_case("enable")
                                    || name_str.eq_ignore_ascii_case("rec out")
                                {
                                    let _ = selem.set_enum_item(SelemChannelId::mono(), idx);
                                    info!("Configured mic control '{}' to '{}' on {}", mic_ctrl, name_str, self.card_name);
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn set_capture_pad(&mut self, profile: &MixerProfile, pad_db: i8) -> Result<(), MixerError> {
        let mixer = self.open_mixer()?;
        let ctrl_name = profile.capture_pad_control_name;
        let selem_id = SelemId::new(ctrl_name, 0);

        if let Some(selem) = mixer.find_selem(&selem_id) {
            if selem.has_capture_volume() {
                // Map dB value to volume slider range in millibels (1/100 dB)
                let (min_db, max_db) = selem.get_capture_db_range();
                let target_db = (pad_db as i64) * 100;
                let clamped_db = target_db.clamp(min_db.0, max_db.0);
                selem.set_capture_db_all(MilliBel(clamped_db), Round::Floor)?;
                info!("Set hardware pad '{}' on {} to {} dB", ctrl_name, self.card_name, pad_db);
                return Ok(());
            } else if selem.is_enumerated() {
                // Some mixers use enum for capture level (e.g. "-19dB", "-15dB", "-10dB", "-5dB")
                let pad_str = format!("{}dB", pad_db);
                if let Ok(count) = selem.get_enum_items() {
                    for idx in 0..count {
                        if let Ok(name) = selem.get_enum_item_name(idx) {
                            if name.contains(&pad_str) {
                                selem.set_enum_item(SelemChannelId::mono(), idx)?;
                                info!("Set hardware pad enum '{}' to index {}", ctrl_name, idx);
                                return Ok(());
                            }
                        }
                    }
                }
            }
        }
        warn!("Capture pad control element '{}' not found on {}", ctrl_name, self.card_name);
        Ok(())
    }

    fn get_capture_pad(&self, profile: &MixerProfile) -> Result<i8, MixerError> {
        let mixer = self.open_mixer()?;
        let ctrl_name = profile.capture_pad_control_name;
        let selem_id = SelemId::new(ctrl_name, 0);

        if let Some(selem) = mixer.find_selem(&selem_id) {
            if selem.has_capture_volume() {
                if let Ok(db_val) = selem.get_capture_vol_db(SelemChannelId::mono()) {
                    return Ok((db_val.0 / 100) as i8);
                }
            }
        }
        Ok(-19) // default if unreadable
    }
}

pub struct MockMixerController {
    pub current_pad: i8,
    pub routing_configured: bool,
    pub set_calls: Vec<i8>,
}

impl Default for MockMixerController {
    fn default() -> Self {
        Self {
            current_pad: -19,
            routing_configured: false,
            set_calls: Vec::new(),
        }
    }
}

impl MixerController for MockMixerController {
    fn configure_rec_out(&mut self, profile: &MixerProfile) -> Result<(), MixerError> {
        if profile.routing_control_name.is_some() {
            self.routing_configured = true;
        }
        Ok(())
    }

    fn set_capture_pad(&mut self, _profile: &MixerProfile, pad_db: i8) -> Result<(), MixerError> {
        self.current_pad = pad_db;
        self.set_calls.push(pad_db);
        Ok(())
    }

    fn get_capture_pad(&self, _profile: &MixerProfile) -> Result<i8, MixerError> {
        Ok(self.current_pad)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use djm_common::profile::find_profile_by_usb_id;

    #[test]
    fn test_mock_mixer_controller() {
        let profile = find_profile_by_usb_id(0x2b73, 0x000a).unwrap(); // DJM-900NXS2
        let mut mock = MockMixerController::default();

        mock.configure_rec_out(&profile).unwrap();
        assert!(mock.routing_configured);

        mock.set_capture_pad(&profile, -15).unwrap();
        assert_eq!(mock.get_capture_pad(&profile).unwrap(), -15);
        assert_eq!(mock.set_calls, vec![-15]);
    }
}

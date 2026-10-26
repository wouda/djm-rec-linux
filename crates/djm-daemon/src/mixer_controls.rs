use alsa::mixer::{MilliBel, Mixer, SelemChannelId, SelemId};
use alsa::Round;
use djm_common::profile::MixerProfile;
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
    fn configure_rec_out(&mut self, profile: &MixerProfile, include_djm_mic: bool) -> Result<(), MixerError>;
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
    fn configure_rec_out(&mut self, profile: &MixerProfile, include_djm_mic: bool) -> Result<(), MixerError> {
        let mixer = match self.open_mixer() {
            Ok(m) => m,
            Err(e) => {
                warn!("Could not open ALSA mixer for {}: {}", self.card_name, e);
                return Err(MixerError::Alsa(e));
            }
        };

        // Configure REC OUT channel pair routing (e.g., 'Input 5' -> 'Rec Out')
        let pair_idx = (profile.rec_out_channels.0 / 2) + 1;
        let input_selem_name = format!("Input {}", pair_idx);
        let input_switch_name = format!("Input {} Capture Switch", pair_idx);
        let ch_input_name = format!("Ch{} Input", pair_idx);
        let target_val = profile.routing_control_value.unwrap_or("Rec Out");

        let mut routing_candidates: Vec<&str> = Vec::new();
        if let Some(ctrl_name) = profile.routing_control_name {
            routing_candidates.push(ctrl_name);
        }
        if !routing_candidates.contains(&input_selem_name.as_str()) {
            routing_candidates.push(&input_selem_name);
        }
        if !routing_candidates.contains(&input_switch_name.as_str()) {
            routing_candidates.push(&input_switch_name);
        }
        if !routing_candidates.contains(&ch_input_name.as_str()) {
            routing_candidates.push(&ch_input_name);
        }

        let mut routed = false;
        for ctrl_name in routing_candidates {
            let selem_id = SelemId::new(ctrl_name, 0);
            if let Some(selem) = mixer.find_selem(&selem_id) {
                if selem.is_enumerated() {
                    if let Ok(count) = selem.get_enum_items() {
                        for idx in 0..count {
                            if let Ok(item_name) = selem.get_enum_item_name(idx) {
                                if item_name.eq_ignore_ascii_case(target_val) {
                                    let _ = selem.set_enum_item(SelemChannelId::mono(), idx);
                                    info!(
                                        "Configured {} '{}' to '{}' (index {})",
                                        self.card_name, ctrl_name, item_name, idx
                                    );
                                    routed = true;
                                    break;
                                }
                            }
                        }
                    }
                }
                if routed {
                    break;
                }
            }
        }
        if !routed {
            warn!(
                "No enumerated REC OUT routing control found on {} (tried pair {})",
                self.card_name, pair_idx
            );
        }

        // Pioneer DJ Mixers: Configure Microphone routing into REC OUT
        let mic_target_switch = if include_djm_mic { 1 } else { 0 };
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
                    let _ = selem.set_capture_switch_all(mic_target_switch);
                    info!(
                        "Set microphone capture switch '{}' to {} on {}",
                        mic_ctrl, mic_target_switch, self.card_name
                    );
                } else if selem.is_enumerated() {
                    if let Ok(count) = selem.get_enum_items() {
                        for idx in 0..count {
                            if let Ok(item_name) = selem.get_enum_item_name(idx) {
                                let name_str = item_name.as_str();
                                let matches = if include_djm_mic {
                                    name_str.eq_ignore_ascii_case("on")
                                        || name_str.eq_ignore_ascii_case("enable")
                                        || name_str.eq_ignore_ascii_case("rec out")
                                } else {
                                    name_str.eq_ignore_ascii_case("off")
                                        || name_str.eq_ignore_ascii_case("disable")
                                        || name_str.eq_ignore_ascii_case("none")
                                };
                                if matches {
                                    let _ = selem.set_enum_item(SelemChannelId::mono(), idx);
                                    info!(
                                        "Configured mic control '{}' to '{}' on {}",
                                        mic_ctrl, name_str, self.card_name
                                    );
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
        let candidates = [
            profile.capture_pad_control_name,
            "Master Input Level",
            "Master Input Level Capture Switch",
            "Capture Level",
            "Capture Level Capture Switch",
        ];

        // Map pad_db (-19, -15, -10, -5) to DJM-V10 / kernel boost notation ("0dB", "+6dB", "+9dB", "+15dB")
        let boost_str = if pad_db <= -18 {
            "0dB"
        } else if pad_db <= -16 {
            "+3dB"
        } else if pad_db <= -13 {
            "+6dB"
        } else if pad_db <= -9 {
            "+9dB"
        } else if pad_db <= -7 {
            "+12dB"
        } else {
            "+15dB"
        };
        let pad_str = format!("{}dB", pad_db);

        for ctrl_name in candidates {
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
                    if let Ok(count) = selem.get_enum_items() {
                        // First pass: direct pad notation (e.g. "-19dB", "-15dB", "-10dB", "-5dB")
                        for idx in 0..count {
                            if let Ok(name) = selem.get_enum_item_name(idx) {
                                if name.eq_ignore_ascii_case(&pad_str) || name.contains(&pad_str) {
                                    selem.set_enum_item(SelemChannelId::mono(), idx)?;
                                    info!(
                                        "Set hardware pad enum '{}' on {} to '{}' (index {})",
                                        ctrl_name, self.card_name, name, idx
                                    );
                                    return Ok(());
                                }
                            }
                        }
                        // Second pass: kernel boost notation (e.g. "0dB" = -19 dB reference, "+6dB", "+9dB", "+15dB")
                        for idx in 0..count {
                            if let Ok(name) = selem.get_enum_item_name(idx) {
                                if name.eq_ignore_ascii_case(boost_str) {
                                    selem.set_enum_item(SelemChannelId::mono(), idx)?;
                                    info!(
                                        "Set hardware pad enum '{}' on {} to '{}' ({} dB pad, index {})",
                                        ctrl_name, self.card_name, name, pad_db, idx
                                    );
                                    return Ok(());
                                }
                            }
                        }
                    }
                }
            }
        }

        warn!(
            "Capture pad control element '{}' not found on {}",
            profile.capture_pad_control_name, self.card_name
        );
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
    pub mic_included: bool,
    pub set_calls: Vec<i8>,
}

impl Default for MockMixerController {
    fn default() -> Self {
        Self {
            current_pad: -19,
            routing_configured: false,
            mic_included: false,
            set_calls: Vec::new(),
        }
    }
}

impl MixerController for MockMixerController {
    fn configure_rec_out(&mut self, profile: &MixerProfile, include_djm_mic: bool) -> Result<(), MixerError> {
        if profile.routing_control_name.is_some() {
            self.routing_configured = true;
        }
        self.mic_included = include_djm_mic;
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

        mock.configure_rec_out(&profile, false).unwrap();
        assert!(mock.routing_configured);
        assert!(!mock.mic_included);

        mock.configure_rec_out(&profile, true).unwrap();
        assert!(mock.mic_included);

        mock.set_capture_pad(&profile, -15).unwrap();
        assert_eq!(mock.get_capture_pad(&profile).unwrap(), -15);
        assert_eq!(mock.set_calls, vec![-15]);
    }
}

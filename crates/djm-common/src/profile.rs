use serde::{Deserialize, Serialize};

pub const VENDOR_PIONEER_LEGACY: u16 = 0x08e4;
pub const VENDOR_ALPHATHETA_PIONEER: u16 = 0x2b73;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MixerProfile {
    pub name: &'static str,
    pub vendor_id: u16,
    pub product_id: u16,
    pub total_channels: u32,
    pub native_sample_rate: u32,
    pub bit_depth: u32,
    /// 0-indexed channel pair (e.g., (8, 9) for Ch 9 & 10)
    pub rec_out_channels: (u32, u32),
    pub routing_control_name: Option<&'static str>,
    pub routing_control_value: Option<&'static str>,
    pub capture_pad_control_name: &'static str,
}

pub static PROFILES: &[MixerProfile] = &[
    MixerProfile {
        name: "Pioneer DJM-900NXS2",
        vendor_id: VENDOR_ALPHATHETA_PIONEER,
        product_id: 0x000a,
        total_channels: 10,
        native_sample_rate: 96000,
        bit_depth: 24,
        rec_out_channels: (8, 9),
        routing_control_name: Some("Ch5 Input"),
        routing_control_value: Some("Rec Out"),
        capture_pad_control_name: "Capture Level",
    },
    MixerProfile {
        name: "AlphaTheta DJM-A9",
        vendor_id: VENDOR_ALPHATHETA_PIONEER,
        product_id: 0x0043,
        total_channels: 12,
        native_sample_rate: 96000,
        bit_depth: 24,
        rec_out_channels: (8, 9),
        routing_control_name: Some("Input 5"),
        routing_control_value: Some("Rec Out"),
        capture_pad_control_name: "Master Input Level",
    },
    MixerProfile {
        name: "Pioneer DJM-V10",
        vendor_id: VENDOR_ALPHATHETA_PIONEER,
        product_id: 0x0034,
        total_channels: 12,
        native_sample_rate: 96000,
        bit_depth: 24,
        rec_out_channels: (8, 9),
        routing_control_name: Some("Input 5"),
        routing_control_value: Some("Rec Out"),
        capture_pad_control_name: "Master Input Level",
    },
    MixerProfile {
        name: "Pioneer DJM-V10",
        vendor_id: VENDOR_ALPHATHETA_PIONEER,
        product_id: 0x002d,
        total_channels: 12,
        native_sample_rate: 96000,
        bit_depth: 24,
        rec_out_channels: (8, 9),
        routing_control_name: Some("Input 5"),
        routing_control_value: Some("Rec Out"),
        capture_pad_control_name: "Master Input Level",
    },
    MixerProfile {
        name: "Pioneer DJM-900NXS",
        vendor_id: VENDOR_PIONEER_LEGACY,
        product_id: 0x015c,
        total_channels: 8,
        native_sample_rate: 48000,
        bit_depth: 24,
        rec_out_channels: (6, 7),
        routing_control_name: None,
        routing_control_value: None,
        capture_pad_control_name: "Capture Level",
    },
    MixerProfile {
        name: "Pioneer DJM-850",
        vendor_id: VENDOR_PIONEER_LEGACY,
        product_id: 0x0163,
        total_channels: 8,
        native_sample_rate: 48000,
        bit_depth: 24,
        rec_out_channels: (6, 7),
        routing_control_name: None,
        routing_control_value: None,
        capture_pad_control_name: "Capture Level",
    },
    MixerProfile {
        name: "Pioneer DJM-750MK2",
        vendor_id: VENDOR_ALPHATHETA_PIONEER,
        product_id: 0x001b,
        total_channels: 8,
        native_sample_rate: 48000,
        bit_depth: 24,
        rec_out_channels: (6, 7),
        routing_control_name: None,
        routing_control_value: None,
        capture_pad_control_name: "Capture Level",
    },
    MixerProfile {
        name: "Pioneer DJM-450",
        vendor_id: VENDOR_ALPHATHETA_PIONEER,
        product_id: 0x0013,
        total_channels: 4,
        native_sample_rate: 48000,
        bit_depth: 24,
        rec_out_channels: (2, 3),
        routing_control_name: None,
        routing_control_value: None,
        capture_pad_control_name: "Capture Level",
    },
    MixerProfile {
        name: "Pioneer DJM-250MK2",
        vendor_id: VENDOR_ALPHATHETA_PIONEER,
        product_id: 0x0017,
        total_channels: 4,
        native_sample_rate: 48000,
        bit_depth: 24,
        rec_out_channels: (2, 3),
        routing_control_name: None,
        routing_control_value: None,
        capture_pad_control_name: "Capture Level",
    },
];

pub fn find_profile_by_usb_id(vendor_id: u16, product_id: u16) -> Option<MixerProfile> {
    PROFILES
        .iter()
        .find(|p| p.vendor_id == vendor_id && p.product_id == product_id)
        .cloned()
}

pub fn is_pioneer_mixer(vendor_id: u16) -> bool {
    vendor_id == VENDOR_PIONEER_LEGACY || vendor_id == VENDOR_ALPHATHETA_PIONEER
}

pub fn generic_pioneer_profile(
    vendor_id: u16,
    product_id: u16,
    detected_channels: u32,
    detected_rate: u32,
) -> MixerProfile {
    let rec_pair = if detected_channels >= 2 {
        (detected_channels - 2, detected_channels - 1)
    } else {
        (0, 1)
    };

    MixerProfile {
        name: "Generic Pioneer DJ USB Mixer",
        vendor_id,
        product_id,
        total_channels: detected_channels.max(2),
        native_sample_rate: if detected_rate > 0 { detected_rate } else { 48000 },
        bit_depth: 24,
        rec_out_channels: rec_pair,
        routing_control_name: None,
        routing_control_value: None,
        capture_pad_control_name: "Capture Level",
    }
}

pub fn match_mixer_from_card_id(card_id: &str) -> Option<MixerProfile> {
    let lower = card_id.to_lowercase();
    if lower.contains("900nxs2") {
        find_profile_by_usb_id(VENDOR_ALPHATHETA_PIONEER, 0x000a)
    } else if lower.contains("a9") {
        find_profile_by_usb_id(VENDOR_ALPHATHETA_PIONEER, 0x0043)
    } else if lower.contains("v10") {
        find_profile_by_usb_id(VENDOR_ALPHATHETA_PIONEER, 0x002d)
    } else if lower.contains("900nxs") {
        find_profile_by_usb_id(VENDOR_PIONEER_LEGACY, 0x015c)
    } else if lower.contains("850") {
        find_profile_by_usb_id(VENDOR_PIONEER_LEGACY, 0x0163)
    } else if lower.contains("750mk2") {
        find_profile_by_usb_id(VENDOR_ALPHATHETA_PIONEER, 0x001b)
    } else if lower.contains("450") {
        find_profile_by_usb_id(VENDOR_ALPHATHETA_PIONEER, 0x0013)
    } else if lower.contains("250mk2") {
        find_profile_by_usb_id(VENDOR_ALPHATHETA_PIONEER, 0x0017)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_known_mixer_profiles() {
        let nxs2 = find_profile_by_usb_id(0x2b73, 0x000a).expect("DJM-900NXS2 not found");
        assert_eq!(nxs2.total_channels, 10);
        assert_eq!(nxs2.native_sample_rate, 96000);
        assert_eq!(nxs2.rec_out_channels, (8, 9));
        assert_eq!(nxs2.routing_control_name, Some("Ch5 Input"));

        let a9 = find_profile_by_usb_id(0x2b73, 0x0043).expect("DJM-A9 not found");
        assert_eq!(a9.native_sample_rate, 96000);
        assert_eq!(a9.rec_out_channels, (8, 9));

        let v10 = find_profile_by_usb_id(0x2b73, 0x0034).expect("DJM-V10 (0x0034) not found");
        assert_eq!(v10.native_sample_rate, 96000);
        assert_eq!(v10.capture_pad_control_name, "Master Input Level");
        assert_eq!(v10.routing_control_name, Some("Input 5"));

        let v10_legacy = find_profile_by_usb_id(0x2b73, 0x002d).expect("DJM-V10 (0x002d) not found");
        assert_eq!(v10_legacy.native_sample_rate, 96000);

        let nxs1 = find_profile_by_usb_id(0x08e4, 0x015c).expect("DJM-900NXS not found");
        assert_eq!(nxs1.native_sample_rate, 48000);
        assert_eq!(nxs1.rec_out_channels, (6, 7));
    }

    #[test]
    fn test_generic_fallback() {
        let generic = generic_pioneer_profile(0x2b73, 0x9999, 8, 96000);
        assert_eq!(generic.vendor_id, 0x2b73);
        assert_eq!(generic.product_id, 0x9999);
        assert_eq!(generic.rec_out_channels, (6, 7));
        assert_eq!(generic.native_sample_rate, 96000);
    }

    #[test]
    fn test_card_id_match() {
        let prof = match_mixer_from_card_id("Pioneer DJM-V10 USB Audio").unwrap();
        assert_eq!(prof.name, "Pioneer DJM-V10");
    }
}

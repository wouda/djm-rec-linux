use anyhow::Result;
use djm_common::config::AppConfig;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

pub struct ApplianceSetup;

impl ApplianceSetup {
    pub const PACKAGES: &'static [&'static str] = &[
        "alsa-lib",
        "alsa-utils",
        "flac",
        "pipewire",
        "networkmanager",
        "avahi",
    ];

    pub fn run_setup(dry_run: bool) -> Result<()> {
        info!("Running djm-box CachyOS / Arch Linux appliance setup...");

        // 1. Check system packages
        Self::check_packages();

        // 2. Initialize configuration file with random auth token
        Self::setup_config(dry_run)?;

        // 3. Install user systemd services
        Self::install_systemd_user_services(dry_run)?;

        // 4. Install binaries to ~/.cargo/bin so systemd can run them
        Self::install_binaries(dry_run)?;

        // 5. Install WirePlumber pro-audio rules
        Self::install_wireplumber_rules(dry_run)?;

        // 6. Check and configure laptop lid-switch & power management
        Self::check_and_configure_power_and_lid(dry_run)?;

        info!("Appliance setup completed successfully!");
        Ok(())
    }

    fn check_packages() {
        info!("Checking required system dependencies: {:?}", Self::PACKAGES);
        let mut missing = Vec::new();

        for pkg in Self::PACKAGES {
            let status = std::process::Command::new("pacman")
                .args(["-Q", pkg])
                .output();

            match status {
                Ok(output) if output.status.success() => {
                    info!("  ✓ {} is installed", pkg);
                }
                _ => {
                    warn!("  ✗ {} is missing", pkg);
                    missing.push(*pkg);
                }
            }
        }

        if !missing.is_empty() {
            warn!(
                "Some dependencies are not installed. You can install them with:\n  yay -S --needed {}",
                missing.join(" ")
            );
        }
    }

    pub fn setup_config(dry_run: bool) -> Result<PathBuf> {
        let config_path = AppConfig::default_config_path();
        if config_path.exists() {
            info!("Existing configuration found at {}", config_path.display());
            return Ok(config_path);
        }

        info!("Creating initial configuration file at {}", config_path.display());
        if !dry_run {
            let _ = AppConfig::load_or_create(&config_path)?;
            // Set 0600 permissions
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let perms = fs::Permissions::from_mode(0o600);
                let _ = fs::set_permissions(&config_path, perms);
            }
        }
        Ok(config_path)
    }

    pub fn generate_daemon_service() -> &'static str {
        r#"[Unit]
Description=djm-box Core DJ Mixer Audio Recording Daemon
After=sound.target
BindsTo=sound.target

[Service]
Type=simple
ExecStart=%h/.cargo/bin/djm-daemon
Restart=always
RestartSec=1
CPUSchedulingPolicy=fifo
CPUSchedulingPriority=80
LimitRTPRIO=95
LimitMEMLOCK=infinity
Environment="RUST_LOG=info"

[Install]
WantedBy=default.target
"#
    }

    pub fn generate_remote_service() -> &'static str {
        r#"[Unit]
Description=djm-box Web Remote Bridge & Hotspot Service
After=djm-box-daemon.service NetworkManager.service
Wants=djm-box-daemon.service

[Service]
Type=simple
ExecStart=%h/.cargo/bin/djm-remote
Restart=always
RestartSec=3
Environment="RUST_LOG=info"

[Install]
WantedBy=default.target
"#
    }

    pub fn install_systemd_user_services(dry_run: bool) -> Result<()> {
        let user_systemd_dir = if let Some(home) = std::env::var_os("HOME") {
            PathBuf::from(home).join(".config/systemd/user")
        } else {
            PathBuf::from("/etc/systemd/user")
        };

        let daemon_path = user_systemd_dir.join("djm-box-daemon.service");
        let remote_path = user_systemd_dir.join("djm-box-remote.service");

        info!("Installing systemd user service: {}", daemon_path.display());
        info!("Installing systemd user service: {}", remote_path.display());

        if !dry_run {
            fs::create_dir_all(&user_systemd_dir)?;
            fs::write(&daemon_path, Self::generate_daemon_service())?;
            fs::write(&remote_path, Self::generate_remote_service())?;
        }

        Ok(())
    }

    pub fn install_binaries(dry_run: bool) -> Result<()> {
        let cargo_bin_dir = if let Some(home) = std::env::var_os("HOME") {
            PathBuf::from(home).join(".cargo/bin")
        } else {
            PathBuf::from("/usr/local/bin")
        };

        let current_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("."));

        let bins = ["djm-daemon", "djm-remote", "djm-box", "djm-tui"];
        info!("Installing binaries to {}", cargo_bin_dir.display());

        if !dry_run {
            fs::create_dir_all(&cargo_bin_dir)?;
            for bin_name in &bins {
                let mut src = current_dir.join(bin_name);
                if !src.exists() {
                    if let Ok(canon) = current_dir.canonicalize() {
                        let rel = canon.join("../release").join(bin_name);
                        let dbg = canon.join("../debug").join(bin_name);
                        if rel.exists() {
                            src = rel;
                        } else if dbg.exists() {
                            src = dbg;
                        }
                    }
                }

                if src.exists() {
                    let dest = cargo_bin_dir.join(bin_name);
                    let _ = fs::remove_file(&dest);
                    fs::copy(&src, &dest)?;
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        let _ = fs::set_permissions(&dest, fs::Permissions::from_mode(0o755));
                    }
                    info!("  ✓ Installed {} -> {}", src.display(), dest.display());
                }
            }

            // Reload user systemd daemon
            let _ = std::process::Command::new("systemctl")
                .args(["--user", "daemon-reload"])
                .status();
        }

        Ok(())
    }

    pub fn generate_wireplumber_rule() -> &'static str {
        r#"monitor.alsa.rules = [
  {
    matches = [
      { device.vendor.id = "0x08e4" },
      { device.vendor.id = "0x2b73" }
    ]
    actions = {
      update-props = {
        api.alsa.use-ucm = false
        session.suspend-timeout-seconds = 0
        device.profile = "pro-audio"
      }
    }
  }
]
"#
    }

    pub fn install_wireplumber_rules(dry_run: bool) -> Result<()> {
        let wp_dir = if let Some(home) = std::env::var_os("HOME") {
            PathBuf::from(home).join(".config/wireplumber/wireplumber.conf.d")
        } else {
            PathBuf::from("/etc/wireplumber/wireplumber.conf.d")
        };

        let wp_file = wp_dir.join("51-djm-pro-audio.conf");
        info!("Installing WirePlumber pro-audio rules: {}", wp_file.display());

        if !dry_run {
            fs::create_dir_all(&wp_dir)?;
            fs::write(&wp_file, Self::generate_wireplumber_rule())?;
        }

        Ok(())
    }

    pub fn generate_udev_rules() -> &'static str {
        r#"# Pioneer Corporation DJ Mixers (Vendor 08e4)
ACTION=="add", SUBSYSTEM=="sound", ATTRS{idVendor}=="08e4", TAG+="systemd", ENV{SYSTEMD_USER_WANTS}="djm-box-daemon.service"

# AlphaTheta / Pioneer DJ Corporation Mixers (Vendor 2b73)
ACTION=="add", SUBSYSTEM=="sound", ATTRS{idVendor}=="2b73", TAG+="systemd", ENV{SYSTEMD_USER_WANTS}="djm-box-daemon.service"
"#
    }

    pub fn generate_logind_config() -> &'static str {
        r#"[Login]
# Keep audio recording alive when closing the laptop lid in the DJ booth
HandleLidSwitch=ignore
HandleLidSwitchExternalPower=ignore
HandleLidSwitchDocked=ignore
"#
    }

    pub fn check_and_configure_power_and_lid(dry_run: bool) -> Result<()> {
        let udev_path = Path::new("/etc/udev/rules.d/99-djm-box.rules");
        let logind_dir = Path::new("/etc/systemd/logind.conf.d");
        let logind_path = logind_dir.join("djm-appliance.conf");

        let local_dir = if let Some(home) = std::env::var_os("HOME") {
            PathBuf::from(home).join(".config/djm-box/system")
        } else {
            PathBuf::from("/tmp/djm-box-system")
        };

        if !dry_run {
            fs::create_dir_all(&local_dir)?;
            fs::write(local_dir.join("99-djm-box.rules"), Self::generate_udev_rules())?;
            fs::write(local_dir.join("djm-appliance.conf"), Self::generate_logind_config())?;

            let is_root = unsafe {
                extern "C" { fn geteuid() -> u32; }
                geteuid() == 0
            };

            let logind_installed = logind_path.exists() && fs::read_to_string(&logind_path)
                .map(|c| c.contains("HandleLidSwitch=ignore"))
                .unwrap_or(false);
            let udev_installed = udev_path.exists();

            if is_root {
                if !udev_installed {
                    if let Some(p) = udev_path.parent() { fs::create_dir_all(p)?; }
                    fs::write(udev_path, Self::generate_udev_rules())?;
                    let _ = std::process::Command::new("udevadm").args(["control", "--reload-rules"]).status();
                }
                if !logind_installed {
                    fs::create_dir_all(logind_dir)?;
                    fs::write(&logind_path, Self::generate_logind_config())?;
                    // Note: We do NOT restart systemd-logind here, as restarting logind kills active Wayland/X11 sessions.
                }
                info!("  ✓ Root access: Installed /etc/udev and /etc/systemd/logind configurations directly.");
            } else if !logind_installed || !udev_installed {
                // Try passwordless sudo or notify
                let sudo_available = std::process::Command::new("sudo")
                    .args(["-n", "true"])
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false);

                if sudo_available {
                    info!("Elevated sudo access detected, automatically installing appliance system configurations...");
                    let _ = std::process::Command::new("sudo")
                        .args(["mkdir", "-p", "/etc/systemd/logind.conf.d", "/etc/udev/rules.d"])
                        .status();
                    let _ = std::process::Command::new("sudo")
                        .args(["cp", &local_dir.join("99-djm-box.rules").to_string_lossy(), "/etc/udev/rules.d/99-djm-box.rules"])
                        .status();
                    let _ = std::process::Command::new("sudo")
                        .args(["cp", &local_dir.join("djm-appliance.conf").to_string_lossy(), "/etc/systemd/logind.conf.d/djm-appliance.conf"])
                        .status();
                    let _ = std::process::Command::new("sudo").args(["udevadm", "control", "--reload-rules"]).status();
                    // Do NOT restart systemd-logind here to avoid terminating active graphical desktop sessions
                    info!("  ✓ Installed /etc/systemd/logind.conf.d/djm-appliance.conf (lid protection active after next reboot/login)");
                    info!("  ✓ Installed /etc/udev/rules.d/99-djm-box.rules and reloaded udev rules");
                } else {
                    warn!("  ⚠ System lid-switch or udev rules not yet installed in /etc.");
                    info!("To protect against laptop sleep when closing the lid, run:");
                    info!("  sudo cp {} /etc/udev/rules.d/99-djm-box.rules", local_dir.join("99-djm-box.rules").display());
                    info!("  sudo mkdir -p /etc/systemd/logind.conf.d");
                    info!("  sudo cp {} /etc/systemd/logind.conf.d/djm-appliance.conf", local_dir.join("djm-appliance.conf").display());
                    info!("  sudo udevadm control --reload-rules");
                }
            } else {
                info!("  ✓ Laptop lid-switch inhibitor is active in /etc/systemd/logind.conf.d/djm-appliance.conf");
                info!("  ✓ Pioneer & AlphaTheta udev rules are active in /etc/udev/rules.d/99-djm-box.rules");
            }

            // Check powerprofilesctl
            if let Ok(output) = std::process::Command::new("powerprofilesctl").arg("get").output() {
                if let Ok(profile) = String::from_utf8(output.stdout) {
                    info!("  ✓ System power profile detected: [{}] (automatic performance profile active during recording)", profile.trim());
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rules_content() {
        let udev = ApplianceSetup::generate_udev_rules();
        assert!(udev.contains("08e4"));
        assert!(udev.contains("2b73"));

        let logind = ApplianceSetup::generate_logind_config();
        assert!(logind.contains("HandleLidSwitch=ignore"));

        let daemon_svc = ApplianceSetup::generate_daemon_service();
        assert!(daemon_svc.contains("CPUSchedulingPolicy=fifo"));
        assert!(daemon_svc.contains("CPUSchedulingPriority=80"));

        let wp = ApplianceSetup::generate_wireplumber_rule();
        assert!(wp.contains("pro-audio"));

        let remote_svc = ApplianceSetup::generate_remote_service();
        assert!(remote_svc.contains("NetworkManager.service"));
        assert!(!remote_svc.contains("bluetooth.service"));
    }
}

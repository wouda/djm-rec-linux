use crate::qr_render::render_qr_to_terminal_string;
use djm_common::protocol::{PairingInfo, RecordingState, Telemetry};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Gauge, Paragraph};
use ratatui::Frame;

pub fn draw_ui(
    f: &mut Frame,
    telemetry: &Telemetry,
    pairing_info: &PairingInfo,
    show_pairing: bool,
    status_msg: Option<&str>,
) {
    let size = f.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4), // Header
            Constraint::Length(8), // Dual Stereo VU Meters
            Constraint::Length(7), // Recording File & Disk Stats
            Constraint::Min(3),    // Status / Recent Logs
            Constraint::Length(2), // Hotkey Footer
        ])
        .split(size);

    draw_header(f, chunks[0], telemetry);
    draw_meters(f, chunks[1], telemetry);
    draw_file_stats(f, chunks[2], telemetry);
    draw_status_logs(f, chunks[3], telemetry, status_msg);
    draw_footer(f, chunks[4]);

    if show_pairing {
        draw_pairing_modal(f, size, pairing_info);
    }
}

fn draw_header(f: &mut Frame, area: Rect, t: &Telemetry) {
    let (rec_badge, rec_style) = match t.state {
        RecordingState::Recording => (
            " ● RECORDING TO DISK ",
            Style::default().fg(Color::Black).bg(Color::Green).add_modifier(Modifier::BOLD),
        ),
        RecordingState::ArmedSilent => (
            " ⏸ ARMED IN RAM (Gate > -65 dBFS) ",
            Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD),
        ),
        RecordingState::Paused => (
            " ⏸ PAUSED ",
            Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD),
        ),
        RecordingState::Disconnected => (
            " ○ IDLE (Not Recording) ",
            Style::default().fg(Color::White).bg(Color::DarkGray),
        ),
    };

    let (mixer_badge, mixer_style) = if t.is_mixer_connected() {
        (
            format!(" [✓ MIXER DETECTED: {} @ {}Hz] ", t.mixer_model, t.sample_rate),
            Style::default().fg(Color::Black).bg(Color::Green).add_modifier(Modifier::BOLD),
        )
    } else {
        (
            " [⚠ NO MIXER DETECTED - Connect USB-B] ".to_string(),
            Style::default().fg(Color::White).bg(Color::Red).add_modifier(Modifier::BOLD),
        )
    };

    let guard_str = if t.auto_guard_active { "Auto-Guard: [ON]" } else { "Auto-Guard: [OFF]" };
    let guard_color = if t.auto_guard_active { Color::Green } else { Color::DarkGray };

    let line1 = Line::from(vec![
        Span::styled(mixer_badge, mixer_style),
        Span::raw("  "),
        Span::styled(rec_badge, rec_style),
    ]);

    let line2 = Line::from(vec![
        Span::styled(format!("Pad: [{} dB]  ", t.hw_pad_db), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled("Mic Routing: [REC OUT (+Mic)]  ", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
        Span::styled(format!("{}  ", guard_str), Style::default().fg(guard_color)),
        Span::styled(format!("Appliance v{}", djm_common::VERSION), Style::default().fg(Color::DarkGray)),
    ]);

    let title = format!(" djm-box v{} DJ Recording Appliance ", djm_common::VERSION);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .style(Style::default().fg(Color::White));

    let p = Paragraph::new(vec![line1, line2]).block(block);
    f.render_widget(p, area);
}

fn draw_meters(f: &mut Frame, area: Rect, t: &Telemetry) {
    let meter_chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(1), // Top legend / 0dB reference
            Constraint::Length(1), // Ch L
            Constraint::Length(1), // Ch R
            Constraint::Length(1), // Silence countdown / Clip warnings
        ])
        .split(area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Real-Time dBFS Peak & RMS Meters (Physical 0dB = -19 dBFS) ")
        .style(Style::default().fg(Color::White));

    f.render_widget(block, area);

    // Legend line with -19 dBFS physical mixer reference
    let legend = Line::from(vec![
        Span::styled("-60 dBFS           ", Style::default().fg(Color::DarkGray)),
        Span::styled("▼ -19 dBFS (Mixer 0 dB REF)", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::styled("            -0.5 dBFS  0 dBFS", Style::default().fg(Color::Red)),
    ]);
    f.render_widget(Paragraph::new(legend), meter_chunks[0]);

    // Gauge calculations: Map -60.0 dBFS (0%) to 0.0 dBFS (100%)
    let pct_l = cdb_to_pct(t.peak_l_centi_db);
    let pct_r = cdb_to_pct(t.peak_r_centi_db);

    let db_str_l = format!("L: {:.1} dBFS", t.peak_l_centi_db as f32 / 100.0);
    let gauge_l = Gauge::default()
        .gauge_style(get_meter_color(pct_l))
        .ratio(pct_l)
        .label(Span::styled(db_str_l, Style::default().fg(Color::White)));
    f.render_widget(gauge_l, meter_chunks[1]);

    let db_str_r = format!("R: {:.1} dBFS", t.peak_r_centi_db as f32 / 100.0);
    let gauge_r = Gauge::default()
        .gauge_style(get_meter_color(pct_r))
        .ratio(pct_r)
        .label(Span::styled(db_str_r, Style::default().fg(Color::White)));
    f.render_widget(gauge_r, meter_chunks[2]);

    // Bottom warning & countdown
    let mut status_spans = Vec::new();
    if t.clip_count > 0 {
        status_spans.push(Span::styled(
            format!(" [! DIGITAL CLIPS: {}] ", t.clip_count),
            Style::default().fg(Color::White).bg(Color::Red).add_modifier(Modifier::BOLD),
        ));
        status_spans.push(Span::raw("  "));
    } else {
        status_spans.push(Span::styled(" CLIPS: 0 (Clean Headroom) ", Style::default().fg(Color::Green)));
        status_spans.push(Span::raw("  "));
    }

    if let Some(countdown) = t.pause_countdown_secs {
        status_spans.push(Span::styled(
            format!("Silence Auto-Split in: {:.0}s", countdown),
            Style::default().fg(Color::Yellow).add_modifier(Modifier::ITALIC),
        ));
    }

    f.render_widget(Paragraph::new(Line::from(status_spans)), meter_chunks[3]);
}

fn draw_file_stats(f: &mut Frame, area: Rect, t: &Telemetry) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Active Recording Segment & Storage ")
        .style(Style::default().fg(Color::White));

    let filename = t.active_filename.as_deref().unwrap_or("No active file on disk");
    let dur_h = t.active_file_duration_secs / 3600;
    let dur_m = (t.active_file_duration_secs / 60) % 60;
    let dur_s = t.active_file_duration_secs % 60;

    let free_gb = t.free_disk_gb_tenths as f32 / 10.0;
    // 24-bit 96kHz stereo FLAC uses ~1.1 GB per hour
    let est_hours_left = if free_gb > 0.0 { free_gb / 1.1 } else { 0.0 };

    let disk_status_span = match t.state {
        RecordingState::Recording => Span::styled("● ACTIVELY WRITING TO DISK", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        RecordingState::ArmedSilent => Span::styled("⏸ ARMED IN RAM (Pre-roll active, gate threshold -65 dBFS)", Style::default().fg(Color::Yellow)),
        RecordingState::Paused => Span::styled("⏸ PAUSED", Style::default().fg(Color::Cyan)),
        RecordingState::Disconnected => Span::styled("○ WAITING FOR MIXER (Connect USB-B cable)", Style::default().fg(Color::Red)),
    };

    let lines = vec![
        Line::from(vec![
            Span::styled("Disk Status:   ", Style::default().fg(Color::DarkGray)),
            disk_status_span,
        ]),
        Line::from(vec![
            Span::styled("File Target:   ", Style::default().fg(Color::DarkGray)),
            Span::styled(filename, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("Duration:      ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:02}:{:02}:{:02}  ", dur_h, dur_m, dur_s), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::styled("Size: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{} MB  ", t.active_file_size_mb), Style::default().fg(Color::Cyan)),
            Span::styled("Disk Free: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:.1} GB (~{:.0} hours remaining)", free_gb, est_hours_left), Style::default().fg(Color::Yellow)),
        ]),
    ];

    f.render_widget(Paragraph::new(lines).block(block), area);
}

fn draw_status_logs(f: &mut Frame, area: Rect, t: &Telemetry, status_msg: Option<&str>) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Status & Session Log ")
        .style(Style::default().fg(Color::DarkGray));

    let mut lines = Vec::new();
    if let Some(msg) = status_msg {
        lines.push(Line::from(Span::styled(format!("> {}", msg), Style::default().fg(Color::Cyan))));
    }

    for log in t.recent_logs.iter().rev().take(4) {
        lines.push(Line::from(Span::styled(format!("  {}", log), Style::default().fg(Color::Gray))));
    }

    f.render_widget(Paragraph::new(lines).block(block), area);
}

fn draw_footer(f: &mut Frame, area: Rect) {
    let hotkeys = Line::from(vec![
        Span::styled("[P]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw(" Pair Phone  "),
        Span::styled("[S]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw(" Split File  "),
        Span::styled("[M]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw(" Bookmark  "),
        Span::styled("[[] / []]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw(" Adjust Pad  "),
        Span::styled("[A]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw(" Auto-Guard  "),
        Span::styled("[R]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw(" Reset Peaks  "),
        Span::styled("[Q]", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
        Span::raw(" Exit TUI"),
    ]);

    f.render_widget(Paragraph::new(hotkeys).alignment(Alignment::Center), area);
}

fn draw_pairing_modal(f: &mut Frame, size: Rect, info: &PairingInfo) {
    let modal_width = 58.min(size.width.saturating_sub(4));
    let modal_height = 29.min(size.height.saturating_sub(2));

    let area = Rect {
        x: (size.width.saturating_sub(modal_width)) / 2,
        y: (size.height.saturating_sub(modal_height)) / 2,
        width: modal_width,
        height: modal_height,
    };

    f.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Phone Pairing & Remote Booth Dashboard ")
        .style(Style::default().fg(Color::Yellow).bg(Color::Black));

    let web_url = info.to_web_url();
    let qr_str = render_qr_to_terminal_string(&web_url).unwrap_or_else(|_| "QR Error".to_string());

    let mut lines = Vec::new();
    lines.push(Line::from(Span::styled("Point Phone Camera at QR to Open Dashboard", Style::default().fg(Color::White).add_modifier(Modifier::BOLD))));
    lines.push(Line::from(vec![
        Span::styled("Direct Wi-Fi:  ", Style::default().fg(Color::DarkGray)),
        Span::styled(&web_url, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
    ]));
    if let Some(ref mdns) = info.mdns_hostname {
        lines.push(Line::from(vec![
            Span::styled("mDNS Hostname: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("http://{}:{}/?token={}", mdns, info.port, info.token), Style::default().fg(Color::Magenta)),
        ]));
    }
    lines.push(Line::from(vec![
        Span::styled("Auth PIN: ", Style::default().fg(Color::DarkGray)),
        Span::styled(&info.token, Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        Span::styled("  |  BLE: ", Style::default().fg(Color::DarkGray)),
        Span::styled(&info.name, Style::default().fg(Color::Yellow)),
    ]));
    lines.push(Line::from(""));

    for l in qr_str.lines() {
        lines.push(Line::from(Span::styled(l, Style::default().fg(Color::White).bg(Color::Black))));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled("Press [Esc] or [P] to close modal", Style::default().fg(Color::DarkGray))));

    let p = Paragraph::new(lines)
        .block(block)
        .alignment(Alignment::Center);

    f.render_widget(p, area);
}

fn cdb_to_pct(cdb: i16) -> f64 {
    // -60 dBFS (or lower) = 0.0, 0 dBFS = 1.0
    let db = cdb as f64 / 100.0;
    if db <= -60.0 {
        0.0
    } else if db >= 0.0 {
        1.0
    } else {
        (db + 60.0) / 60.0
    }
}

fn get_meter_color(pct: f64) -> Style {
    if pct > 0.95 {
        // Red (> -3 dBFS)
        Style::default().fg(Color::Red)
    } else if pct > 0.68 {
        // Yellow (> -19 dBFS, above physical mixer 0 dB reference)
        Style::default().fg(Color::Yellow)
    } else {
        // Green
        Style::default().fg(Color::Green)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_meter_scaling() {
        assert_eq!(cdb_to_pct(-6000), 0.0);
        assert_eq!(cdb_to_pct(0), 1.0);
        // -19 dBFS should be around 0.683
        let pct_19 = cdb_to_pct(-1900);
        assert!(pct_19 > 0.65 && pct_19 < 0.70);
    }
}

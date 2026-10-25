use qrcode::types::Color;
use qrcode::QrCode;

/// Renders a QR code as a multi-line string using Unicode half-block characters (`▀`, `▄`, `█`, ` `).
/// Each terminal line renders two vertical pixels.
pub fn render_qr_to_terminal_string(data: &str) -> Result<String, qrcode::types::QrError> {
    let code = QrCode::new(data.as_bytes())?;
    let width = code.width();
    let modules = code.into_colors();

    let mut out = String::new();
    let border = 2;
    let full_width = width + border * 2;

    // We process 2 rows at a time
    for y in (0..(width + border * 2)).step_by(2) {
        for x in 0..full_width {
            let top_black = is_black(&modules, width, border, x, y);
            let bottom_black = is_black(&modules, width, border, x, y + 1);

            // In standard dark terminal, black module on white background:
            // invert for dark mode so QR modules stand out brightly on dark background
            let char_to_draw = match (top_black, bottom_black) {
                (true, true) => '█',
                (true, false) => '▀',
                (false, true) => '▄',
                (false, false) => ' ',
            };
            out.push(char_to_draw);
        }
        out.push('\n');
    }

    Ok(out)
}

fn is_black(modules: &[Color], width: usize, border: usize, x: usize, y: usize) -> bool {
    if x < border || x >= width + border || y < border || y >= width + border {
        return false;
    }
    let orig_x = x - border;
    let orig_y = y - border;
    modules[orig_y * width + orig_x] == Color::Dark
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_qr_generation() {
        let qr_str = render_qr_to_terminal_string("djm-box:849201").expect("failed to render QR");
        assert!(!qr_str.is_empty());
        assert!(qr_str.contains('█') || qr_str.contains('▀') || qr_str.contains('▄'));
    }
}

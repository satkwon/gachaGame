//! Inline image rendering via the Kitty graphics protocol, which Ghostty
//! implements. Images are transmitted as base64-encoded PNG in chunked APC
//! escape sequences and scaled into a cell box with the `c=` / `r=` keys.

use std::io::Write;

use base64::{engine::general_purpose::STANDARD, Engine};

/// Best-effort detection of a terminal that speaks the Kitty graphics protocol.
pub fn supported() -> bool {
    if std::env::var_os("WISH_FORCE_GRAPHICS").is_some() {
        return true;
    }
    if std::env::var_os("GHOSTTY_RESOURCES_DIR").is_some()
        || std::env::var_os("GHOSTTY_BIN_DIR").is_some()
        || std::env::var_os("KITTY_WINDOW_ID").is_some()
    {
        return true;
    }
    let term = std::env::var("TERM").unwrap_or_default();
    let prog = std::env::var("TERM_PROGRAM").unwrap_or_default();
    term.contains("kitty")
        || term.contains("ghostty")
        || prog.eq_ignore_ascii_case("ghostty")
        || prog.eq_ignore_ascii_case("kitty")
        || prog.eq_ignore_ascii_case("wezterm")
}

/// Transmit and display a PNG at the current cursor position, scaled into a box
/// of `cols` x `rows` terminal cells. The cursor is left where it started; the
/// caller decides how much vertical space to reserve.
pub fn draw_png<W: Write>(out: &mut W, png: &[u8], cols: u16, rows: u16) -> std::io::Result<()> {
    let encoded = STANDARD.encode(png);
    let bytes = encoded.as_bytes();
    let chunk = 4096;
    let total = bytes.len();
    let mut offset = 0;
    let mut first = true;

    while offset < total {
        let end = (offset + chunk).min(total);
        let is_last = end == total;
        let m = if is_last { 0 } else { 1 };

        out.write_all(b"\x1b_G")?;
        if first {
            // a=T transmit+display, f=100 PNG, C=1 do not move the cursor.
            write!(out, "a=T,f=100,c={cols},r={rows},C=1,m={m};")?;
            first = false;
        } else {
            write!(out, "m={m};")?;
        }
        out.write_all(&bytes[offset..end])?;
        out.write_all(b"\x1b\\")?;
        offset = end;
    }
    out.flush()?;
    Ok(())
}

/// Delete every image currently placed on screen.
pub fn clear<W: Write>(out: &mut W) -> std::io::Result<()> {
    out.write_all(b"\x1b_Ga=d\x1b\\")?;
    out.flush()
}

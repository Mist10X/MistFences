//! Icon labels rendered the way Explorer's desktop list view renders them, for labels composed
//! on the CPU (MistFences' desktop icon layer, presented with `UpdateLayeredWindow`).
//!
//! Layout follows the desktop (see [`layout_lines`]): the icon-title font
//! (`SPI_GETICONTITLELOGFONT`), centred lines, breaks between words only, at most two lines,
//! "..." where a word or the text does not fit.
//! GDI draws each line white on black with the system's smoothing (ClearType when it is on, as
//! on the desktop); the result is per-subpixel coverage, which the caller lays over the label's
//! shadow.

use crate::bindings::*;
use crate::wide::to_wide;
use windows_core::PCWSTR;

const SPI_GETFONTSMOOTHING: u32 = 0x004A;
const SPI_GETFONTSMOOTHINGTYPE: u32 = 0x200A;
const FE_FONTSMOOTHINGCLEARTYPE: u32 = 2;
const ELLIPSIS: &str = "...";

windows_core::link!("gdi32.dll" "system" fn GetTextExtentPoint32W(hdc: HDC, text: PCWSTR, len: i32, size: *mut SIZE) -> windows_core::BOOL);

/// A rendered label: `width` x `height` pixels, each with red, green and blue coverage
/// (0 = none, 255 = full; equal channels without ClearType).
pub struct LabelText {
    pub width: i32,
    pub height: i32,
    /// Height of one line (the font's cell height).
    pub line_height: i32,
    pub lines: Vec<String>,
    pub coverage: Vec<[u8; 3]>,
}

/// Whether text is smoothed with ClearType right now (the user's setting).
pub fn cleartype_on() -> bool {
    let mut on: u32 = 0;
    let mut kind: u32 = 0;
    // SAFETY: both calls write one u32 each.
    unsafe {
        let _ = SystemParametersInfoW(SPI_GETFONTSMOOTHING, 0, (&mut on as *mut u32).cast(), 0);
        let _ = SystemParametersInfoW(
            SPI_GETFONTSMOOTHINGTYPE,
            0,
            (&mut kind as *mut u32).cast(),
            0,
        );
    }
    on != 0 && kind == FE_FONTSMOOTHINGCLEARTYPE
}

/// Explorer's label lines for `text`, and the label's text width: `measure` gives a string's
/// width in pixels, `wrap` is the widest a line may be (the cell less the label's margins).
///
/// Measured against Explorer's desktop list view (Windows 11, 75 px cells, Segoe UI 9 pt):
/// the label is as wide as its widest line when the whole text is wrapped at `wrap` (a word
/// wider than that counts as the part of it that fits); lines break between words only. The
/// first line is cut with "..." inside that width when its first word is too wide, and the rest
/// of that word is dropped. The last line takes all the text left, ending in "..." inside that
/// width when it does not fit: so "Архив Chatterino by Mist1X" shows "Chatteri...", in the 56 px
/// of "Chatterino", not "Chatterino b..." in the 71 px a line may have.
pub fn layout_lines(
    text: &str,
    wrap: i32,
    max_lines: usize,
    measure: &dyn Fn(&str) -> i32,
) -> (Vec<String>, i32) {
    // The longest start of `s` that fits in `room` together with the ellipsis.
    let cut = |s: &str, room: i32| -> String {
        let chars: Vec<char> = s.chars().collect();
        (1..=chars.len())
            .rev()
            .map(|n| chars[..n].iter().collect::<String>().trim_end().to_string() + ELLIPSIS)
            .find(|c| measure(c) <= room)
            .unwrap_or_else(|| ELLIPSIS.to_string())
    };
    let fitting_part = |word: &str| -> i32 {
        let chars: Vec<char> = word.chars().collect();
        (1..=chars.len())
            .rev()
            .map(|n| measure(&chars[..n].iter().collect::<String>()))
            .find(|w| *w <= wrap)
            .unwrap_or(0)
    };
    let words: Vec<&str> = text.split(' ').filter(|w| !w.is_empty()).collect();
    // Greedy word wrap from `i`: the line and the index after it.
    let wrap_from = |i: usize| -> (String, usize) {
        let mut line = words[i].to_string();
        let mut j = i + 1;
        while j < words.len() {
            let next = format!("{line} {}", words[j]);
            if measure(&next) > wrap {
                break;
            }
            line = next;
            j += 1;
        }
        (line, j)
    };
    let mut width = 0;
    let mut i = 0;
    while i < words.len() {
        if measure(words[i]) > wrap {
            width = width.max(fitting_part(words[i]));
            i += 1;
        } else {
            let (line, j) = wrap_from(i);
            width = width.max(measure(&line));
            i = j;
        }
    }
    let mut lines = Vec::new();
    let mut i = 0;
    while i < words.len() && lines.len() < max_lines {
        if lines.len() + 1 == max_lines {
            let rest = words[i..].join(" ");
            lines.push(if measure(&rest) <= width {
                rest
            } else {
                cut(&rest, width)
            });
            break;
        }
        if measure(words[i]) > wrap {
            lines.push(cut(words[i], width));
            i += 1;
        } else {
            let (line, j) = wrap_from(i);
            lines.push(line);
            i = j;
        }
    }
    (lines, width)
}

/// Renders `text` as an Explorer desktop label at `px` pixels em height (the icon-title font
/// `family` / `weight` at the monitor's DPI) for a cell `width` pixels wide, at most
/// `max_lines` lines. The label keeps `margin` pixels (2 at 96 DPI) on both sides of its text
/// and is centred under the icon box, one pixel narrower than the cell; each line is centred
/// in the label. None when GDI fails.
#[allow(clippy::too_many_arguments)]
pub fn render_label(
    text: &str,
    family: &str,
    px: i32,
    weight: i32,
    width: i32,
    margin: i32,
    max_lines: usize,
    cleartype: bool,
) -> Option<LabelText> {
    if width <= 2 * margin + 1 || px <= 0 || max_lines == 0 {
        return None;
    }
    let face = to_wide(family);
    // SAFETY: every GDI object created here is selected out and deleted before returning; the
    // DIB's bits are read only after GdiFlush, while the bitmap is alive.
    unsafe {
        let screen = GetDC(None);
        let mem = CreateCompatibleDC(Some(screen));
        let quality = if cleartype {
            CLEARTYPE_QUALITY
        } else {
            ANTIALIASED_QUALITY
        };
        let font = CreateFontW(
            -px,
            0,
            0,
            0,
            weight,
            0,
            0,
            0,
            DEFAULT_CHARSET as u32,
            0,
            0,
            quality as u32,
            0,
            PCWSTR(face.as_ptr()),
        );
        let old_font = SelectObject(mem, HGDIOBJ(font.0));
        let measure = |s: &str| -> i32 {
            let w = to_wide(s);
            let mut size = SIZE::default();
            let _ = GetTextExtentPoint32W(
                mem,
                PCWSTR(w.as_ptr()),
                w.len().saturating_sub(1) as i32,
                &mut size,
            );
            size.cx
        };
        let line_height = {
            let w = to_wide("Ay");
            let mut size = SIZE::default();
            let _ = GetTextExtentPoint32W(mem, PCWSTR(w.as_ptr()), 2, &mut size);
            size.cy.max(1)
        };
        let (lines, text_width) = layout_lines(text, width - 2 * margin, max_lines, &measure);
        // Explorer's label rectangle (the text plus its margins) centred under the icon box.
        let text_left = ((width - 1 - (text_width + 2 * margin)) / 2).max(0) + margin;
        let height = line_height * lines.len().max(1) as i32;
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB as u32,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits: *mut core::ffi::c_void = core::ptr::null_mut();
        let dib = CreateDIBSection(
            Some(screen),
            &info,
            DIB_RGB_COLORS as u32,
            &mut bits,
            None,
            0,
        );
        let _ = ReleaseDC(None, screen);
        if dib.0.is_null() || bits.is_null() {
            SelectObject(mem, old_font);
            let _ = DeleteObject(HGDIOBJ(font.0));
            let _ = DeleteDC(mem);
            return None;
        }
        let old_bitmap = SelectObject(mem, HGDIOBJ(dib.0));
        SetTextColor(mem, COLORREF(0x00FF_FFFF));
        SetBkMode(mem, TRANSPARENT);
        for (i, line) in lines.iter().enumerate() {
            let w = to_wide(line);
            let top = line_height * i as i32;
            let mut rect = RECT {
                left: text_left + (text_width - measure(line)).max(0) / 2,
                top,
                right: width,
                bottom: top + line_height,
            };
            let format = DT_LEFT as u32 | DT_SINGLELINE as u32 | DT_NOPREFIX as u32;
            DrawTextW(
                mem,
                PCWSTR(w.as_ptr()),
                w.len().saturating_sub(1) as i32,
                &mut rect,
                format,
            );
        }
        let _ = GdiFlush();
        let pixels = core::slice::from_raw_parts(bits.cast::<u8>(), (width * height * 4) as usize);
        // DIB pixels are B, G, R, x.
        let coverage = pixels
            .as_chunks::<4>()
            .0
            .iter()
            .map(|p| [p[2], p[1], p[0]])
            .collect();
        SelectObject(mem, old_bitmap);
        SelectObject(mem, old_font);
        let _ = DeleteObject(HGDIOBJ(dib.0));
        let _ = DeleteObject(HGDIOBJ(font.0));
        let _ = DeleteDC(mem);
        Some(LabelText {
            width,
            height,
            line_height,
            lines,
            coverage,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fixed-width "font": 7 px per character.
    fn mono(s: &str) -> i32 {
        s.chars().count() as i32 * 7
    }

    fn lines(text: &str) -> Vec<String> {
        layout_lines(text, 70, 2, &mono).0
    }

    #[test]
    fn breaks_like_explorers_desktop() {
        // A word too wide for the line is cut and ends its line; the rest goes below.
        assert_eq!(lines("Counter-Strike 2"), ["Counter...", "2"]);
        // A single word too wide: one line.
        assert_eq!(lines("abcdefghijklmnop"), ["abcdefg..."]);
        // Words that fit together share a line.
        assert_eq!(lines("Logitech G HUB"), ["Logitech G", "HUB"]);
        assert_eq!(lines("Telegram"), ["Telegram"]);
        // The last line takes the rest, cut to the label's width (its widest wrapped line,
        // "Chatterino" here), not to the width a line may have.
        assert_eq!(lines("Архив Chatterino by Mist1X"), ["Архив", "Chatter..."]);
        assert_eq!(
            layout_lines("Ab Cd Ef Gh Ij Kl Mn Op Qr", 70, 2, &mono),
            (vec!["Ab Cd Ef".to_string(), "Gh Ij...".to_string()], 56)
        );
        // Never more lines than allowed, never wider than the line.
        for name in [
            "MistFences — техническое исследование",
            "Перезагрузить с VPN",
            "Дополнительные сведения об этом изображении",
        ] {
            let (got, width) = layout_lines(name, 70, 2, &mono);
            assert!(got.len() <= 2, "{name}: {got:?}");
            assert!(
                width <= 70 && got.iter().all(|l| mono(l) <= width),
                "{name}: {got:?}"
            );
        }
    }

    /// Explorer's own lines for these names on Windows 11 (75 px cells, Segoe UI 9 pt at 96
    /// DPI), read off its desktop in Windows Sandbox.
    #[test]
    fn segoe_ui_labels_match_explorer() {
        for (name, want) in [
            ("Counter-Strike 2", &["Counter-Str...", "2"][..]),
            ("Перезагрузить с VPN", &["Перезагру...", "с VPN"]),
            ("Архив Chatterino by Mist1X", &["Архив", "Chatteri..."]),
            (
                "MistFences — техническое исследование",
                &["MistFences", "— техниче..."],
            ),
            (
                "Steam Deck Docking Station",
                &["Steam Deck", "Docking S..."],
            ),
            (
                "Ab Cd Ef Gh Ij Kl Mn Op Qr",
                &["Ab Cd Ef Gh", "Ij Kl Mn O..."],
            ),
            ("abcdefghijklmnopqrstuvwxyz", &["abcdefghij..."]),
            ("Документы и таблицы 2026", &["Документы", "и таблиц..."]),
            ("Один два три четыре пять", &["Один два", "три четы..."]),
            (
                "Alpha Bravo Charlie Delta",
                &["Alpha Bravo", "Charlie Delta"],
            ),
            (
                "Visual Studio Code Insiders",
                &["Visual Studio", "Code Insiders"],
            ),
            ("Новая папка (2) копия", &["Новая папка", "(2) копия"]),
            ("Battlefield™ 6", &["Battlefield™ 6"]),
            ("Logitech G HUB", &["Logitech G", "HUB"]),
        ] {
            let label = render_label(name, "Segoe UI", 12, 400, 75, 2, 2, true).unwrap();
            assert_eq!(label.lines, want, "{name}");
        }
    }

    #[test]
    fn renders_cyrillic_and_latin_with_some_coverage() {
        for (name, cleartype) in [("Перезагрузить с VPN", true), ("Telegram", false)]
        {
            let label = render_label(name, "Segoe UI", 12, 400, 75, 2, 2, cleartype).unwrap();
            assert!(label.lines.len() <= 2);
            let lit = label
                .coverage
                .iter()
                .filter(|c| c.iter().any(|v| *v > 128))
                .count();
            assert!(lit > 20, "{name}: {lit} lit pixels");
            if !cleartype {
                assert!(label.coverage.iter().all(|c| c[0] == c[1] && c[1] == c[2]));
            }
        }
    }
}

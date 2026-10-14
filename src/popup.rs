//! Drawing the suggestion popup into the terminal, and restoring what was
//! underneath from the vt100 model of the screen.

use std::io::Write;

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::config::{Ui, sgr_color};
use crate::engine::{Kind, Suggestion};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub row: u16,
    pub col: u16,
    pub height: u16,
    pub width: u16,
}

pub struct Layout {
    pub rect: Rect,
    icon_w: u16,
    name_w: u16,
    desc_w: u16,
}

struct Theme {
    bg: String,
    fg: String,
    sel_bg: String,
    sel_fg: String,
    desc_fg: String,
    match_fg: String,
    bar_fg: String,
}

fn theme(ui: &Ui) -> Theme {
    Theme {
        bg: sgr_color(&ui.bg, 48),
        fg: sgr_color(&ui.fg, 38),
        sel_bg: sgr_color(&ui.selected_bg, 48),
        sel_fg: sgr_color(&ui.selected_fg, 38),
        desc_fg: sgr_color(&ui.description_fg, 38),
        match_fg: sgr_color(&ui.match_fg, 38),
        bar_fg: sgr_color(&ui.scrollbar_fg, 38),
    }
}

fn icon(kind: Kind, style: &str) -> (&'static str, &'static str) {
    // (glyph, SGR foreground)
    let color = match kind {
        Kind::Command => "38;5;213",
        Kind::Subcommand => "38;5;81",
        Kind::Option => "38;5;221",
        Kind::Arg => "38;5;114",
        Kind::File => "38;5;250",
        Kind::Folder => "38;5;75",
        Kind::Shortcut => "38;5;209",
    };
    let glyph = match style {
        "none" => "",
        "nerd" => match kind {
            Kind::Command => "\u{f120}",
            Kind::Subcommand => "\u{f0da}",
            Kind::Option => "\u{f024}",
            Kind::Arg => "\u{f0eb}",
            Kind::File => "\u{f15b}",
            Kind::Folder => "\u{f07b}",
            Kind::Shortcut => "\u{f0e7}",
        },
        _ => match kind {
            Kind::Command => "❯",
            Kind::Subcommand => "▸",
            Kind::Option => "◆",
            Kind::Arg => "•",
            Kind::File => "▫",
            Kind::Folder => "▪",
            Kind::Shortcut => "★",
        },
    };
    (glyph, color)
}

/// Truncate to `w` columns (adding `…` when cut) and pad to exactly `w`.
fn fit(s: &str, w: usize) -> String {
    let mut out = String::new();
    let mut used = 0;
    if s.width() <= w {
        out.push_str(s);
        used = s.width();
    } else if w > 0 {
        for c in s.chars() {
            let cw = c.width().unwrap_or(0);
            if used + cw > w - 1 {
                break;
            }
            out.push(c);
            used += cw;
        }
        out.push('…');
        used += 1;
    }
    out.extend(std::iter::repeat_n(' ', w.saturating_sub(used)));
    out
}

/// Where the popup goes for this screen/cursor, or `None` if it can't fit.
pub fn layout(
    ui: &Ui,
    items: &[Suggestion],
    (rows, cols): (u16, u16),
    (crow, ccol): (u16, u16),
    token_width: u16,
) -> Option<Layout> {
    if items.is_empty() || rows < 3 || cols < 12 {
        return None;
    }
    let icon_w: u16 = if ui.icons == "none" { 0 } else { 2 };
    let name_w = items.iter().map(|s| s.label.width()).max().unwrap_or(1).clamp(4, 40) as u16;
    let desc_max = if ui.descriptions { items.iter().map(|s| s.description.width()).max().unwrap_or(0) } else { 0 };
    // pad + icon + name + (gap + desc) + pad + scrollbar
    let fixed = 1 + icon_w + name_w + 2;
    let max_w = ui.max_width.min(cols).max(fixed + 2);
    let desc_w = if desc_max > 0 { (desc_max as u16).min(max_w.saturating_sub(fixed + 2)) } else { 0 };
    let width = (fixed + if desc_w > 0 { 2 + desc_w } else { 0 }).min(cols);
    let name_w = name_w.min(width.saturating_sub(1 + icon_w + 2));

    let want = (items.len() as u16).min(ui.max_rows.max(1));
    let below = rows.saturating_sub(crow + 1);
    let above = crow;
    let (row, height) = if below >= want {
        (crow + 1, want)
    } else if above >= want {
        (crow - want, want)
    } else if below >= above {
        (crow + 1, below)
    } else {
        (crow - above, above)
    };
    if height == 0 {
        return None;
    }
    // Align the suggestion text with the start of the token being completed.
    let anchor = ccol.saturating_sub(token_width).saturating_sub(1 + icon_w);
    let col = anchor.min(cols.saturating_sub(width));
    Some(Layout { rect: Rect { row, col, height, width }, icon_w, name_w, desc_w })
}

pub fn draw(out: &mut Vec<u8>, ui: &Ui, l: &Layout, items: &[Suggestion], selected: usize, scroll: usize) {
    let t = theme(ui);
    let r = l.rect;
    let n = items.len();
    let h = r.height as usize;
    // Scrollbar thumb.
    let (thumb_start, thumb_len) = if n > h {
        let len = (h * h).div_ceil(n).max(1);
        let start = (scroll * h) / n;
        (start.min(h - len), len)
    } else {
        (0, 0)
    };
    for i in 0..h {
        let idx = scroll + i;
        let Some(s) = items.get(idx) else { break };
        let sel = idx == selected;
        let (bg, fg) = if sel { (&t.sel_bg, &t.sel_fg) } else { (&t.bg, &t.fg) };
        let _ = write!(out, "\x1b[{};{}H\x1b[0;{bg};{fg}m ", r.row as usize + i + 1, r.col + 1);
        if l.icon_w > 0 {
            let (glyph, color) = icon(s.kind, &ui.icons);
            let color = if sel { fg.as_str() } else { color };
            let _ = write!(out, "\x1b[{color}m{}\x1b[{fg}m", fit(glyph, l.icon_w as usize));
        }
        // Label, with the typed prefix highlighted.
        let label = fit(&s.label, l.name_w as usize);
        let hl = if !s.query.is_empty() && s.label.to_lowercase().starts_with(&s.query.to_lowercase()) {
            s.query.chars().count().min(label.chars().count())
        } else {
            0
        };
        let split = label.char_indices().nth(hl).map_or(label.len(), |(b, _)| b);
        let (a, b) = label.split_at(split);
        if hl > 0 && !sel {
            let _ = write!(out, "\x1b[1;{}m{a}\x1b[22;{fg}m{b}", t.match_fg);
        } else if hl > 0 {
            let _ = write!(out, "\x1b[1m{a}\x1b[22m{b}");
        } else {
            out.extend_from_slice(label.as_bytes());
        }
        if l.desc_w > 0 {
            let dfg = if sel { fg } else { &t.desc_fg };
            let _ = write!(out, "  \x1b[{dfg}m{}\x1b[{fg}m", fit(&s.description, l.desc_w as usize));
        }
        let used = 1 + l.icon_w + l.name_w + if l.desc_w > 0 { 2 + l.desc_w } else { 0 };
        let pad = r.width.saturating_sub(used + 1) as usize;
        out.extend(std::iter::repeat_n(b' ', pad));
        let bar = if thumb_len > 0 && i >= thumb_start && i < thumb_start + thumb_len { "▐" } else { " " };
        let _ = write!(out, "\x1b[0;{};{}m{bar}\x1b[0m", t.bg, t.bar_fg);
    }
}

/// Repaint the screen rows covered by `rect` from the vt100 model.
pub fn erase(out: &mut Vec<u8>, screen: &vt100::Screen, rect: Rect) {
    let (_, cols) = screen.size();
    let range = rect.row..rect.row + rect.height;
    for (i, row) in screen.rows_formatted(0, cols).enumerate() {
        let i = i as u16;
        if range.contains(&i) {
            let _ = write!(out, "\x1b[{};1H\x1b[0m\x1b[2K", i + 1);
            out.extend_from_slice(&row);
        } else if i >= range.end {
            break;
        }
    }
}

/// Put the cursor and SGR state back where the shell left them.
pub fn restore_cursor(out: &mut Vec<u8>, screen: &vt100::Screen) {
    let (r, c) = screen.cursor_position();
    let _ = write!(out, "\x1b[0m\x1b[{};{}H", r + 1, c + 1);
    out.extend_from_slice(&screen.attributes_formatted());
    if !screen.hide_cursor() {
        out.extend_from_slice(b"\x1b[?25h");
    }
}

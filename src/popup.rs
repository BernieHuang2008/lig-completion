//! Drawing the suggestion popup into the terminal, and restoring what was
//! underneath from the vt100 model of the screen.

use std::io::Write;

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::config::Ui;
use crate::engine::{Kind, Suggestion};
use crate::theme::{self, Theme};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub row: u16,
    pub col: u16,
    pub height: u16,
    pub width: u16,
}

/// Border characters: top-left, top-right, bottom-left, bottom-right,
/// horizontal, vertical.
type Border = [&'static str; 6];
const ROUNDED: Border = ["╭", "╮", "╰", "╯", "─", "│"];
const SQUARE: Border = ["┌", "┐", "└", "┘", "─", "│"];

#[derive(Clone, Copy)]
struct Style {
    border: Option<Border>,
    pad_l: u16,
    pad_r: u16,
    /// Blank rows above and below the list.
    pad_v: u16,
    /// Between the name and description columns.
    gap: u16,
    /// Spaces after the icon.
    icon_gap: u16,
    /// Cap on the description column.
    desc_max: u16,
}

fn style(ui: &Ui) -> Style {
    let border = match ui.border.as_str() {
        "none" => None,
        "square" => Some(SQUARE),
        _ => Some(ROUNDED),
    };
    match ui.density.as_str() {
        "compact" => Style { border, pad_l: 1, pad_r: 1, pad_v: 0, gap: 2, icon_gap: 1, desc_max: 40 },
        "relaxed" => Style { border, pad_l: 3, pad_r: 3, pad_v: 1, gap: 6, icon_gap: 2, desc_max: u16::MAX },
        _ => Style { border, pad_l: 2, pad_r: 2, pad_v: 0, gap: 4, icon_gap: 1, desc_max: u16::MAX },
    }
}

pub struct Layout {
    pub rect: Rect,
    style: Style,
    icon_w: u16,
    name_w: u16,
    desc_w: u16,
}

impl Layout {
    fn b(&self) -> u16 {
        u16::from(self.style.border.is_some())
    }

    /// Border and padding rows.
    fn chrome_h(&self) -> u16 {
        2 * (self.b() + self.style.pad_v)
    }

    /// Rows available for suggestions.
    pub fn visible(&self) -> usize {
        (self.rect.height - self.chrome_h()) as usize
    }
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

/// Rows below the cursor (besides the border) that are enough to keep the
/// popup there.
const MIN_BELOW: u16 = 4;
/// A description column narrower than this isn't worth showing.
const MIN_DESC: u16 = 8;

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
    let style = style(ui);
    let b = u16::from(style.border.is_some());
    let icon_w: u16 = if ui.icons == "none" { 0 } else { 1 + style.icon_gap };
    // Without a border the scrollbar takes its own column.
    let bar = 1 - b;
    let chrome_w = 2 * b + style.pad_l + style.pad_r + icon_w + bar;
    let name_need = (items.iter().map(|s| s.label.width()).max().unwrap_or(1) as u16).clamp(4, 44);
    let desc_need = if ui.descriptions {
        (items.iter().map(|s| s.description.width()).max().unwrap_or(0) as u16).min(style.desc_max)
    } else {
        0
    };

    // Width: fixed (`width = 60` / `"50%"`) or fit to the content within
    // `min_width..=max_width`.
    let smallest = (chrome_w + 4).min(cols);
    let max_w = ui.max_width.resolve(cols).unwrap_or(cols).clamp(smallest, cols);
    let content_w = chrome_w + name_need + if desc_need > 0 { style.gap + desc_need } else { 0 };
    let width = match ui.width.resolve(cols) {
        Some(w) => w,
        None => content_w.clamp(ui.min_width.min(max_w), max_w),
    }
    .clamp(smallest, cols);
    // Space goes to the name first; the description gets the rest, or is
    // dropped when it wouldn't be readable.
    let inner = width - chrome_w;
    let mut name_w = name_need.min(inner);
    let mut desc_w = 0;
    if desc_need > 0 && inner >= name_w + style.gap + MIN_DESC.min(desc_need) {
        desc_w = inner - name_w - style.gap;
    } else {
        name_w = inner;
    }

    // Height: rows of suggestions (or a share of the terminal, border
    // included), at least `min_height`.
    let chrome_h = 2 * (b + style.pad_v);
    let max_items = match ui.height.resolve(rows) {
        Some(h) if ui.height.is_percent() => h.saturating_sub(chrome_h),
        Some(h) => h,
        None => 8,
    }
    .max(1);
    let want = (items.len() as u16).min(max_items).max(ui.min_height.min(max_items)) + chrome_h;
    let below = rows.saturating_sub(crow + 1);
    let above = crow;
    // Prefer below the cursor, shrinking (the list scrolls); only flip above
    // when there's hardly any room below.
    let (row, height) = if below >= want || below >= want.min(MIN_BELOW + chrome_h) || below >= above {
        (crow + 1, want.min(below))
    } else {
        let h = want.min(above);
        (crow - h, h)
    };
    if height <= chrome_h {
        return None;
    }
    // Align the suggestion text with the start of the token being completed.
    let anchor = ccol.saturating_sub(token_width).saturating_sub(b + style.pad_l + icon_w);
    let col = anchor.min(cols.saturating_sub(width));
    Some(Layout { rect: Rect { row, col, height, width }, style, icon_w, name_w, desc_w })
}

/// The popup as lines of text with SGR codes, each exactly `rect.width` wide.
pub fn render(ui: &Ui, l: &Layout, items: &[Suggestion], selected: usize, scroll: usize) -> Vec<String> {
    let t: Theme = theme::resolve(ui);
    let nerd = crate::icons::use_nerd(&ui.icons);
    let st = l.style;
    let w = l.rect.width as usize;
    let n = items.len();
    let h = l.visible();
    let mut lines = Vec::with_capacity(l.rect.height as usize);

    // Scrollbar thumb.
    let (thumb_start, thumb_len) = if n > h {
        let len = (h * h).div_ceil(n).max(1);
        let start = (scroll * h) / n;
        (start.min(h - len), len)
    } else {
        (0, 0)
    };
    let edge = format!("\x1b[0;{};{}m", t.border_bg, t.border_fg);

    if let Some(bd) = st.border {
        lines.push(format!("{edge}{}{}{}\x1b[0m", bd[0], bd[4].repeat(w - 2), bd[1]));
    }
    // A blank row inside the box (relaxed density).
    let blank = |t: &Theme| match st.border {
        Some(bd) => format!("{edge}{}\x1b[0;{}m{}{edge}{}\x1b[0m", bd[5], t.bg, " ".repeat(w - 2), bd[5]),
        None => format!("\x1b[0;{}m{}\x1b[0m", t.bg, " ".repeat(w)),
    };
    for _ in 0..st.pad_v {
        lines.push(blank(&t));
    }
    for i in 0..h {
        let idx = scroll + i;
        let mut line = String::new();
        if let Some(bd) = st.border {
            line.push_str(&format!("{edge}{}", bd[5]));
        }
        let sel = idx == selected;
        let (bg, fg, dfg) = if sel { (&t.sel_bg, &t.sel_fg, &t.sel_desc_fg) } else { (&t.bg, &t.fg, &t.desc_fg) };
        line.push_str(&format!("\x1b[0;{bg};{fg}m{}", " ".repeat(st.pad_l as usize)));
        match items.get(idx) {
            Some(s) => {
                if l.icon_w > 0 {
                    let (glyph, color) = crate::icons::glyph(s.kind, s.icon, &s.label, nerd);
                    let color = if s.kind == Kind::Info { t.info_fg.as_str() } else { color };
                    line.push_str(&format!("\x1b[{color}m{}\x1b[{fg}m", fit(glyph, l.icon_w as usize)));
                }
                let label = fit(&s.label, l.name_w as usize);
                if s.kind == Kind::Info {
                    line.push_str(&format!("\x1b[3;{}m{label}\x1b[23;{fg}m", t.info_fg));
                } else {
                    // The typed prefix, highlighted.
                    let hl = if !s.query.is_empty() && s.label.to_lowercase().starts_with(&s.query.to_lowercase()) {
                        s.query.chars().count().min(label.chars().count())
                    } else {
                        0
                    };
                    let split = label.char_indices().nth(hl).map_or(label.len(), |(b, _)| b);
                    let (a, b) = label.split_at(split);
                    if hl > 0 {
                        let mfg = if sel { fg } else { &t.match_fg };
                        line.push_str(&format!("\x1b[1;{mfg}m{a}\x1b[22;{fg}m{b}"));
                    } else {
                        line.push_str(&label);
                    }
                }
                if l.desc_w > 0 {
                    line.push_str(&format!(
                        "{}\x1b[{dfg}m{}\x1b[{fg}m",
                        " ".repeat(st.gap as usize),
                        fit(&s.description, l.desc_w as usize)
                    ));
                }
            }
            None => {
                let inner = l.icon_w + l.name_w + if l.desc_w > 0 { st.gap + l.desc_w } else { 0 };
                line.push_str(&" ".repeat(inner as usize));
            }
        }
        line.push_str(&" ".repeat(st.pad_r as usize));
        let thumb = thumb_len > 0 && i >= thumb_start && i < thumb_start + thumb_len;
        match st.border {
            Some(_) if thumb => line.push_str(&format!("\x1b[0;{};{}m┃", t.border_bg, t.thumb_fg)),
            Some(bd) => line.push_str(&format!("{edge}{}", bd[5])),
            None if thumb => line.push_str(&format!("\x1b[0;{};{}m▐", t.bg, t.thumb_fg)),
            None if thumb_len > 0 => line.push_str(&format!("\x1b[0;{};{}m▐", t.bg, t.track_fg)),
            None => line.push_str(&format!("\x1b[0;{}m ", t.bg)),
        }
        line.push_str("\x1b[0m");
        lines.push(line);
    }
    for _ in 0..st.pad_v {
        lines.push(blank(&t));
    }
    if let Some(bd) = st.border {
        // Position counter in the bottom edge when the list scrolls.
        let counter = if n > h && selected < n { format!(" {}/{} ", selected + 1, n) } else { String::new() };
        let fill = w.saturating_sub(2 + counter.width() + usize::from(!counter.is_empty()));
        let tail = if counter.is_empty() { String::new() } else { format!("\x1b[{}m{counter}{edge}{}", t.info_fg, bd[4]) };
        lines.push(format!("{edge}{}{}{tail}{}\x1b[0m", bd[2], bd[4].repeat(fill), bd[3]));
    }
    lines
}

/// `lig preview`: a sample popup under a fake prompt, as text.
pub fn preview(ui: &Ui, cols: u16) -> String {
    use crate::spec::Icon;
    let s = |label: &str, kind, icon, desc: &str, query: &str| Suggestion {
        label: label.into(),
        description: desc.into(),
        kind,
        icon,
        insert: String::new(),
        delete: 0,
        back: 0,
        query: query.into(),
    };
    let items = vec![
        s("checkout", Kind::Subcommand, None, "Switch branches or restore working tree files", "ch"),
        s("cherry-pick", Kind::Subcommand, None, "Apply the changes introduced by some existing commits", "ch"),
        s("main", Kind::Arg, Some(Icon::GitBranch), "fix login redirect (2 hours ago)", ""),
        s("--force-with-lease", Kind::Option, None, "Force, but only if the remote is as expected", ""),
        s("src/", Kind::Folder, None, "", ""),
        s("Cargo.toml", Kind::File, None, "modified", ""),
        s("react", Kind::Arg, Some(Icon::Npm), "React is a JavaScript library for building user interfaces", ""),
        s("nginx", Kind::Arg, Some(Icon::Docker), "official · Official build of Nginx", ""),
        s("numpy", Kind::Arg, Some(Icon::Python), "801.6M downloads/month", ""),
    ];
    let prompt = "% git ch";
    let Some(l) = layout(ui, &items, (40, cols), (0, prompt.width() as u16), 2) else { return String::new() };
    let mut out = format!("{prompt}\x1b[7m \x1b[0m\n");
    for line in render(ui, &l, &items, 1, 0) {
        out.push_str(&format!("{}{line}\n", " ".repeat(l.rect.col as usize)));
    }
    out
}

pub fn draw(out: &mut Vec<u8>, ui: &Ui, l: &Layout, items: &[Suggestion], selected: usize, scroll: usize) {
    let r = l.rect;
    for (i, line) in render(ui, l, items, selected, scroll).iter().enumerate() {
        let _ = write!(out, "\x1b[{};{}H{line}", r.row as usize + i + 1, r.col + 1);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn items(n: usize) -> Vec<Suggestion> {
        (0..n)
            .map(|i| Suggestion {
                label: format!("item{i}"),
                description: format!("description {i}"),
                kind: Kind::Arg,
                icon: None,
                insert: String::new(),
                delete: 0,
                back: 0,
                query: String::new(),
            })
            .collect()
    }

    fn strip(s: &str) -> String {
        let mut out = String::new();
        let mut esc = false;
        for c in s.chars() {
            match (esc, c) {
                (false, '\x1b') => esc = true,
                (true, c) if c.is_ascii_alphabetic() => esc = false,
                (true, _) => {}
                (false, c) => out.push(c),
            }
        }
        out
    }

    #[test]
    fn placement() {
        let ui = Ui::default();
        let at = |crow| layout(&ui, &items(20), (24, 80), (crow, 4), 0).unwrap().rect;
        // Plenty of room: below, 8 rows + border.
        assert_eq!((at(2).row, at(2).height), (3, 10));
        // Mid-screen with 7 rows left: stays below, shrinks.
        assert_eq!((at(16).row, at(16).height), (17, 7));
        // Last line: above.
        assert_eq!((at(23).row, at(23).height), (13, 10));
        let ui = Ui { border: "none".into(), ..Ui::default() };
        let r = layout(&ui, &items(20), (24, 80), (2, 4), 0).unwrap().rect;
        assert_eq!(r.height, 8);
    }

    #[test]
    fn rounded_border_and_counter() {
        let ui = Ui { icons: "none".into(), ..Ui::default() };
        let it = items(20);
        let l = layout(&ui, &it, (24, 80), (2, 10), 0).unwrap();
        let lines: Vec<String> = render(&ui, &l, &it, 2, 0).iter().map(|s| strip(s)).collect();
        assert!(lines[0].starts_with('╭') && lines[0].ends_with('╮'));
        assert!(lines[1].starts_with("│  item0") && lines[1].contains("description 0"));
        // Relaxed: blank rows inside the border, wider gaps.
        let rui = Ui { icons: "none".into(), density: "relaxed".into(), ..Ui::default() };
        let rl = layout(&rui, &it, (24, 80), (2, 10), 0).unwrap();
        let relaxed: Vec<String> = render(&rui, &rl, &it, 0, 0).iter().map(|s| strip(s)).collect();
        assert_eq!(relaxed[1].trim_matches(['│', ' ']), "");
        assert!(relaxed[2].starts_with("│   item0      "));
        assert_eq!(rl.visible(), 8);
        assert!(lines.last().unwrap().starts_with('╰') && lines.last().unwrap().contains(" 3/20 "));
        for line in &lines {
            assert_eq!(line.width(), l.rect.width as usize, "{line:?}");
        }
        // Compact, square, no border.
        for (border, density) in
            [("square", "compact"), ("rounded", "relaxed"), ("none", "relaxed"), ("none", "compact"), ("none", "normal")]
        {
            let ui = Ui { border: border.into(), density: density.into(), ..Ui::default() };
            let l = layout(&ui, &it, (24, 80), (2, 10), 0).unwrap();
            for line in render(&ui, &l, &it, 0, 0) {
                assert_eq!(strip(&line).width(), l.rect.width as usize, "{border}/{density}: {line:?}");
            }
        }
    }

    #[test]
    fn width_and_height_settings() {
        use crate::config::Size;
        let it = items(20);
        let base = Ui { icons: "none".into(), ..Ui::default() };
        // Fixed width, in columns or percent; the description takes the extra room.
        let ui = Ui { width: Size::Cells(70), ..base.clone() };
        let l = layout(&ui, &it, (24, 100), (2, 10), 0).unwrap();
        assert_eq!(l.rect.width, 70);
        assert!(render(&ui, &l, &it, 0, 0).iter().all(|r| strip(r).width() == 70));
        let ui = Ui { width: Size::Text("50%".into()), ..base.clone() };
        assert_eq!(layout(&ui, &it, (24, 120), (2, 10), 0).unwrap().rect.width, 60);
        // Narrow fixed width: the description goes before the name shrinks.
        let ui = Ui { width: Size::Cells(20), ..base.clone() };
        let l = layout(&ui, &it, (24, 100), (2, 10), 0).unwrap();
        assert!(strip(&render(&ui, &l, &it, 0, 0)[1]).contains("item0") && l.desc_w == 0);
        // Auto width respects min/max.
        let ui = Ui { min_width: 60, ..base.clone() };
        assert_eq!(layout(&ui, &it, (24, 100), (2, 10), 0).unwrap().rect.width, 60);
        let ui = Ui { max_width: Size::Text("25%".into()), ..base.clone() };
        assert_eq!(layout(&ui, &it, (24, 100), (2, 10), 0).unwrap().rect.width, 25);
        // Height: rows, percent (border included), and padding with min_height.
        let ui = Ui { height: Size::Cells(4), ..base.clone() };
        assert_eq!(layout(&ui, &it, (40, 100), (2, 10), 0).unwrap().visible(), 4);
        let ui = Ui { height: Size::Text("50%".into()), ..base.clone() };
        assert_eq!(layout(&ui, &it, (40, 100), (2, 10), 0).unwrap().rect.height, 20);
        let ui = Ui { min_height: 6, ..base.clone() };
        let few = items(2);
        let l = layout(&ui, &few, (40, 100), (2, 10), 0).unwrap();
        assert_eq!(l.visible(), 6);
        assert_eq!(render(&ui, &l, &few, 0, 0).len(), 8);
    }

    #[test]
    fn every_theme_resolves() {
        for name in crate::theme::NAMES {
            let ui = Ui { theme: name.to_string(), ..Ui::default() };
            let t = crate::theme::resolve(&ui);
            assert!(!t.bg.is_empty() && !t.sel_bg.is_empty());
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

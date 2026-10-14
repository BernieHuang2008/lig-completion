//! Splitting terminal input into keys and matching configured bindings.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Key {
    Up,
    Down,
    Left,
    Right,
    Tab,
    ShiftTab,
    Enter,
    Esc,
    Ctrl(char),
    CtrlSpace,
    Other,
}

impl Key {
    pub fn parse_name(name: &str) -> Option<Key> {
        let n = name.trim().to_ascii_lowercase();
        Some(match n.as_str() {
            "up" => Key::Up,
            "down" => Key::Down,
            "left" => Key::Left,
            "right" => Key::Right,
            "tab" => Key::Tab,
            "shift-tab" | "backtab" => Key::ShiftTab,
            "enter" | "return" => Key::Enter,
            "esc" | "escape" => Key::Esc,
            "ctrl-space" | "ctrl-@" => Key::CtrlSpace,
            _ => {
                let c = n.strip_prefix("ctrl-")?;
                let mut it = c.chars();
                let ch = it.next().filter(|c| c.is_ascii_lowercase())?;
                if it.next().is_some() {
                    return None;
                }
                Key::Ctrl(ch)
            }
        })
    }
}

/// Split an input chunk into `(bytes, key)` pairs.
pub fn split(input: &[u8]) -> Vec<(&[u8], Key)> {
    let mut out = vec![];
    let mut i = 0;
    while i < input.len() {
        let rest = &input[i..];
        let (len, key) = one(rest);
        let len = len.max(1).min(rest.len());
        out.push((&rest[..len], key));
        i += len;
    }
    out
}

fn one(s: &[u8]) -> (usize, Key) {
    match s[0] {
        0x1b => {
            if s.len() == 1 {
                return (1, Key::Esc);
            }
            match s[1] {
                b'[' => {
                    if s[2..].starts_with(b"200~") {
                        // Bracketed paste: one opaque chunk.
                        let end = find(s, b"\x1b[201~").map_or(s.len(), |p| p + 6);
                        return (end, Key::Other);
                    }
                    let Some(fin) = s[2..].iter().position(|b| (0x40..=0x7e).contains(b)) else {
                        return (s.len(), Key::Other);
                    };
                    let len = 2 + fin + 1;
                    let params = &s[2..2 + fin];
                    let key = match (params, s[2 + fin]) {
                        (b"", b'A') => Key::Up,
                        (b"", b'B') => Key::Down,
                        (b"", b'C') => Key::Right,
                        (b"", b'D') => Key::Left,
                        (b"", b'Z') => Key::ShiftTab,
                        _ => Key::Other,
                    };
                    (len, key)
                }
                b'O' if s.len() >= 3 => {
                    let key = match s[2] {
                        b'A' => Key::Up,
                        b'B' => Key::Down,
                        b'C' => Key::Right,
                        b'D' => Key::Left,
                        _ => Key::Other,
                    };
                    (3, key)
                }
                // Alt+key
                _ => (1 + utf8_len(&s[1..]), Key::Other),
            }
        }
        0x00 => (1, Key::CtrlSpace),
        b'\t' => (1, Key::Tab),
        b'\r' | b'\n' => (1, Key::Enter),
        c @ 0x01..=0x1a => (1, Key::Ctrl((b'a' + c - 1) as char)),
        _ => (utf8_len(s), Key::Other),
    }
}

fn utf8_len(s: &[u8]) -> usize {
    match s.first() {
        Some(b) if b & 0xe0 == 0xc0 => 2,
        Some(b) if b & 0xf0 == 0xe0 => 3,
        Some(b) if b & 0xf8 == 0xf0 => 4,
        _ => 1,
    }
}

fn find(h: &[u8], n: &[u8]) -> Option<usize> {
    h.windows(n.len()).position(|w| w == n)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Accept,
    Next,
    Prev,
    Dismiss,
    Toggle,
}

pub struct Bindings(Vec<(Key, Action)>);

impl Bindings {
    pub fn new(k: &crate::config::Keys) -> Self {
        let mut v = vec![];
        for (names, action) in [
            (&k.accept, Action::Accept),
            (&k.next, Action::Next),
            (&k.prev, Action::Prev),
            (&k.dismiss, Action::Dismiss),
            (&k.toggle, Action::Toggle),
        ]
        {
            for n in names {
                match Key::parse_name(n) {
                    Some(key) => v.push((key, action)),
                    None => eprintln!("lig: unknown key name {n:?} in config"),
                }
            }
        }
        Bindings(v)
    }

    pub fn action(&self, key: &Key) -> Option<Action> {
        if *key == Key::Other {
            return None;
        }
        self.0.iter().find(|(k, _)| k == key).map(|(_, a)| *a)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits() {
        let keys: Vec<Key> = split(b"a\x1b[A\x1bOB\t\x1b[Z\x00\x1b").into_iter().map(|(_, k)| k).collect();
        assert_eq!(keys, vec![Key::Other, Key::Up, Key::Down, Key::Tab, Key::ShiftTab, Key::CtrlSpace, Key::Esc]);
        let parts = split("é\x1bx\x0e".as_bytes());
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[2].1, Key::Ctrl('n'));
    }
}

//! Small scalar-based editor. Chinese widths are explicit; unsupported glyphs
//! are displayed as ASCII escapes, never guessed into a full grapheme algorithm.
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};
pub(super) const LIMIT: usize = 64 * 1024;
#[derive(Debug, PartialEq)]
pub(super) enum Key {
    Text(String),
    Paste(String),
    Clipboard,
    Tab,
    Escape,
    Left,
    Right,
    Home,
    End,
    Backspace,
    Delete,
    Clear,
    Kill,
    Previous,
    Next,
    Enter,
    Eof,
    Cancel,
    TooLong,
}
#[derive(Default)]
pub(super) struct Keys {
    bytes: Vec<u8>,
    paste: Option<Vec<u8>>,
    overflow: bool,
    escape_at: Option<Instant>,
}
impl Keys {
    pub fn feed(&mut self, input: &[u8]) -> Vec<Key> {
        let mut keys = Vec::new();
        if self.expire_escape() {
            keys.push(Key::Escape);
        }
        for &byte in input {
            if let Some(paste) = &mut self.paste {
                paste.push(byte);
                if paste.ends_with(b"\x1b[201~") {
                    if self.overflow {
                        keys.push(Key::TooLong);
                    } else {
                        keys.push(Key::Paste(
                            String::from_utf8_lossy(&paste[..paste.len() - 6])
                                .replace("\r\n", "\n")
                                .replace('\r', "\n"),
                        ));
                    }
                    self.paste = None;
                    self.overflow = false;
                } else if paste.len() > LIMIT + 6 || self.overflow {
                    self.overflow = true;
                    let length = paste.len();
                    paste.drain(..length.saturating_sub(6));
                }
                continue;
            }
            self.bytes.push(byte);
            if self.bytes[0] == 27 {
                self.escape_at = Some(Instant::now());
                if self.bytes.len() == 1 {
                    continue;
                }
                if self.bytes.len() == 2 && matches!(byte, b'[' | b'O') {
                    continue;
                }
                if self.bytes.len() >= 3 && !(0x40..=0x7e).contains(&byte) && self.bytes.len() < 32 {
                    continue;
                }
                let key = match self.bytes.as_slice() {
                    b"\x1b[D" | b"\x1bOD" => Some(Key::Left),
                    b"\x1b[C" | b"\x1bOC" => Some(Key::Right),
                    b"\x1b[H" | b"\x1bOH" | b"\x1b[1~" | b"\x1b[7~" => Some(Key::Home),
                    b"\x1b[F" | b"\x1bOF" | b"\x1b[4~" | b"\x1b[8~" => Some(Key::End),
                    b"\x1b[3~" => Some(Key::Delete),
                    b"\x1b[A" => Some(Key::Previous),
                    b"\x1b[B" => Some(Key::Next),
                    b"\x1b[200~" => {
                        self.paste = Some(Vec::new());
                        None
                    }
                    _ => None,
                };
                keys.extend(key);
                self.bytes.clear();
                self.escape_at = None;
                continue;
            }
            if self.bytes.len() == 1 && byte < 128 {
                let key = match byte {
                    3 => Some(Key::Cancel),
                    4 => Some(Key::Eof),
                    10 | 13 => Some(Key::Enter),
                    8 | 127 => Some(Key::Backspace),
                    9 => Some(Key::Tab),
                    1 => Some(Key::Home),
                    5 => Some(Key::End),
                    2 => Some(Key::Left),
                    6 => Some(Key::Right),
                    21 => Some(Key::Clear),
                    22 => Some(Key::Clipboard),
                    11 => Some(Key::Kill),
                    32..=126 => Some(Key::Text(char::from(byte).to_string())),
                    _ => None,
                };
                keys.extend(key);
                self.bytes.clear();
                continue;
            }
            match std::str::from_utf8(&self.bytes) {
                Ok(text) => {
                    keys.push(Key::Text(text.into()));
                    self.bytes.clear();
                }
                Err(error) if error.error_len().is_some() => {
                    keys.push(Key::Text("\u{fffd}".into()));
                    self.bytes.clear();
                }
                Err(_) => {}
            }
        }
        keys
    }
    pub fn expire_escape(&mut self) -> bool {
        if self.escape_at.is_some_and(|start| start.elapsed() >= Duration::from_millis(100)) {
            self.bytes.clear();
            self.escape_at = None;
            return true;
        }
        false
    }
}
#[derive(Default)]
pub(super) struct Editor {
    pub text: String,
    pub cursor: usize,
    history: VecDeque<String>,
    selected: Option<usize>,
    draft: String,
    completion_selected: usize,
    completion_dismissed: bool,
    pub approval_pending: bool,
    pastes: Vec<(String, String)>,
}
pub(super) fn catalog(platform: &str) -> Vec<(String, String, Vec<String>)> {
    let entries: serde_json::Value =
        serde_json::from_str(include_str!("../../../web/commands.json")).expect("valid shared slash command catalog");
    entries
        .as_array()
        .expect("command catalog array")
        .iter()
        .filter(|entry| {
            entry["platforms"].as_array().is_some_and(|values| values.iter().any(|value| value == platform))
        })
        .map(|entry| {
            (
                entry["name"].as_str().unwrap().into(),
                entry["description"].as_str().unwrap().into(),
                entry["arguments"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter_map(|value| value.as_str().map(str::to_owned))
                    .collect(),
            )
        })
        .collect()
}
impl Editor {
    pub fn suggestions(&self) -> Vec<(String, String)> {
        if self.completion_dismissed
            || self.cursor != self.text.len()
            || !self.text.starts_with('/')
            || self.text.contains(['\n', '\r', '\t'])
        {
            return Vec::new();
        }
        let entries = catalog("console");
        if let Some((name, argument)) = self.text.split_once(' ') {
            if argument.contains(' ') {
                return Vec::new();
            }
            return entries
                .into_iter()
                .filter(|entry| entry.0 == name)
                .flat_map(|entry| {
                    entry
                        .2
                        .into_iter()
                        .filter(|value| value.starts_with(argument))
                        .map(move |value| (format!("{name} {value}"), entry.1.clone()))
                })
                .collect();
        }
        entries
            .into_iter()
            .filter(|entry| {
                entry.0.starts_with(&self.text)
                    && (self.approval_pending || !["/approve", "/deny"].contains(&entry.0.as_str()))
            })
            .map(|entry| (entry.0, entry.1))
            .collect()
    }
    pub fn completion_index(&self) -> usize {
        self.completion_selected.min(self.suggestions().len().saturating_sub(1))
    }
    pub fn completion_key(&mut self, key: &Key) -> bool {
        if matches!(key, Key::Escape) {
            self.completion_dismissed = true;
            return true;
        }
        let suggestions = self.suggestions();
        if suggestions.is_empty() {
            return false;
        }
        match key {
            Key::Previous => {
                self.completion_selected = (self.completion_index() + suggestions.len() - 1) % suggestions.len()
            }
            Key::Next => self.completion_selected = (self.completion_index() + 1) % suggestions.len(),
            Key::Tab | Key::Enter => {
                let selected = suggestions[self.completion_index()].0.clone();
                let has_arguments = catalog("console").iter().any(|entry| entry.0 == selected && !entry.2.is_empty());
                if matches!(key, Key::Enter) && selected == self.text && (self.text.contains(' ') || !has_arguments) {
                    return false;
                }
                self.text = selected;
                if has_arguments {
                    self.text.push(' ');
                }
                self.cursor = self.text.len();
                self.completion_selected = 0;
                self.completion_dismissed = !has_arguments;
            }
            _ => return false,
        }
        true
    }
    pub fn paste(&mut self, text: &str) -> bool {
        let clean: String = text.chars().filter(|c| !c.is_control() || matches!(c, '\n' | '\t')).collect();
        let inserted = if clean.chars().count() >= 1000 || clean.contains('\n') {
            let token = format!("[Pasted Content #{} {} chars]", self.pastes.len() + 1, clean.chars().count());
            self.pastes.push((token.clone(), clean));
            if self.insert(&token) {
                true
            } else {
                self.pastes.pop();
                false
            }
        } else {
            self.insert(&clean)
        };
        self.completion_dismissed = true;
        inserted
    }
    fn expanded(&self, visible: &str) -> String {
        let mut result = String::new();
        let mut offset = 0;
        while offset < visible.len() {
            let next = self
                .pastes
                .iter()
                .filter_map(|(token, text)| visible[offset..].find(token).map(|i| (i + offset, token, text)))
                .min_by_key(|(i, _, _)| *i);
            let Some((i, token, text)) = next else {
                result.push_str(&visible[offset..]);
                break;
            };
            result.push_str(&visible[offset..i]);
            result.push_str(text);
            offset = i + token.len();
        }
        result
    }
    pub fn insert(&mut self, text: &str) -> bool {
        let text: String = text.chars().filter(|c| !c.is_control() || matches!(c, '\n' | '\t')).collect();
        let mut proposed = self.text.clone();
        proposed.insert_str(self.cursor, &text);
        if self.expanded(&proposed).len() > LIMIT {
            return false;
        }
        self.text.insert_str(self.cursor, &text);
        self.cursor += text.len();
        self.completion_dismissed = false;
        self.completion_selected = 0;
        true
    }
    pub fn edit(&mut self, key: Key) {
        self.completion_dismissed = false;
        self.completion_selected = 0;
        if matches!(key, Key::Backspace | Key::Delete)
            && let Some((start, end)) = self
                .pastes
                .iter()
                .filter_map(|(token, _)| {
                    self.text.match_indices(token).find_map(|(start, _)| {
                        let end = start + token.len();
                        ((matches!(key, Key::Backspace) && end == self.cursor)
                            || (matches!(key, Key::Delete) && start == self.cursor))
                            .then_some((start, end))
                    })
                })
                .next()
        {
            self.text.replace_range(start..end, "");
            self.cursor = start;
            return;
        }
        let previous = self.text[..self.cursor].char_indices().next_back().map_or(0, |(i, _)| i);
        let next = self.text[self.cursor..].chars().next().map_or(self.cursor, |c| self.cursor + c.len_utf8());
        match key {
            Key::Left => self.cursor = previous,
            Key::Right => self.cursor = next,
            Key::Home => self.cursor = 0,
            Key::End => self.cursor = self.text.len(),
            Key::Backspace => {
                self.text.replace_range(previous..self.cursor, "");
                self.cursor = previous;
            }
            Key::Delete | Key::Eof => {
                self.text.replace_range(self.cursor..next, "");
            }
            Key::Clear => {
                self.text.clear();
                self.cursor = 0;
            }
            Key::Kill => self.text.truncate(self.cursor),
            Key::Previous if !self.history.is_empty() => {
                if self.selected.is_none() {
                    self.draft = self.text.clone();
                }
                let index = self.selected.unwrap_or(self.history.len()).saturating_sub(1);
                self.selected = Some(index);
                self.text = self.history[index].clone();
                self.cursor = self.text.len();
            }
            Key::Next => {
                if let Some(index) = self.selected {
                    self.selected = (index + 1 < self.history.len()).then_some(index + 1);
                    self.text = self.selected.map_or_else(|| self.draft.clone(), |i| self.history[i].clone());
                    self.cursor = self.text.len();
                }
            }
            _ => {}
        }
    }
    #[cfg(test)]
    pub fn submit(&mut self) -> String {
        self.submit_without(&[])
    }
    pub fn submit_without(&mut self, tokens: &[String]) -> String {
        self.completion_dismissed = false;
        self.completion_selected = 0;
        self.cursor = 0;
        self.selected = None;
        self.draft.clear();
        let mut visible = self.text.clone();
        for token in tokens {
            visible = visible.replace(token, "");
        }
        let text = self.expanded(&visible);
        self.text.clear();
        self.pastes.clear();
        if !text.trim().is_empty()
            && !["y", "n", "/approve", "/deny"].contains(&text.to_lowercase().as_str())
            && !matches!(text.split_whitespace().next(), Some("/mode"))
            && self.history.back() != Some(&text)
        {
            self.history.push_back(text.clone());
            if self.history.len() > 50 {
                self.history.pop_front();
            }
        }
        text
    }
}
pub(super) fn glyph(c: char) -> (String, usize) {
    if c == '\n' {
        return ("\\n".into(), 2);
    }
    if c == '\t' {
        return ("\\t".into(), 2);
    }
    if matches!(c as u32, 0x302a..=0x302f | 0x3099..=0x309a) {
        return escape(c);
    }
    if matches!(c as u32, 0x1100..=0x115f | 0x2329..=0x232a | 0x2e80..=0x303e | 0x3040..=0xa4cf |
        0xac00..=0xd7a3 | 0xf900..=0xfaff | 0xfe10..=0xfe19 | 0xfe30..=0xfe6f | 0xff01..=0xff60 |
        0xffe0..=0xffe6 | 0x20000..=0x3fffd)
    {
        return (c.to_string(), 2);
    }
    if c.is_ascii_graphic() || c == ' ' || matches!(c as u32, 0x00c0..=0x02af) {
        return (c.to_string(), 1);
    }
    escape(c)
}
fn escape(c: char) -> (String, usize) {
    let text = format!("\\u{{{:x}}}", c as u32);
    let width = text.len();
    (text, width)
}
// Never fill the final terminal column: this prevents an implicit wrap on redraw.
pub(super) fn view(editor: &Editor, tail: &str, columns: usize) -> (String, usize) {
    let available = columns.saturating_sub(1);
    let mut prefix = String::new();
    let mut used = 0;
    let mut suffix = Vec::new();
    for c in tail.chars().rev() {
        let (text, width) = glyph(c);
        if used + width > available / 3 {
            break;
        }
        used += width;
        suffix.push(text);
    }
    for text in suffix.into_iter().rev() {
        prefix.push_str(&text);
    }
    let prompt = if used > 0 {
        " | > "
    } else if available >= 7 {
        "lninl> "
    } else {
        "> "
    };
    for c in prompt.chars().take(available.saturating_sub(used)) {
        prefix.push(c);
        used += 1;
    }
    let capacity = available.saturating_sub(used);
    let left = if editor.cursor == editor.text.len() { capacity.saturating_sub(1) } else { capacity / 2 };
    let mut start = editor.cursor;
    let mut width = 0;
    for (i, c) in editor.text[..editor.cursor].char_indices().rev() {
        let n = glyph(c).1;
        if width + n > left {
            break;
        }
        width += n;
        start = i;
    }
    let cursor = used + width;
    for c in editor.text[start..].chars() {
        let (text, n) = glyph(c);
        if used + n > available {
            break;
        }
        prefix.push_str(&text);
        used += n;
    }
    (prefix, cursor)
}

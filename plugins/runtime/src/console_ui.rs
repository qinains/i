use super::{
    console_editor::{Editor, Key, Keys, glyph, view},
    markdown, terminal_mode,
};
use std::{
    collections::VecDeque,
    io::{self, BufRead, IsTerminal, Read, Write},
    sync::mpsc,
    thread,
    time::Duration,
};
pub(super) enum Input {
    Line(String),
    Attachments { line: String, images: Vec<Vec<u8>> },
    Clipboard,
    Eof,
    Cancel,
}
enum RawInput {
    Bytes(Vec<u8>),
    Line(String),
    Eof,
}
enum Output {
    Bytes(usize, Vec<u8>),
    End(usize),
}
pub(super) struct Sink {
    sender: mpsc::SyncSender<Output>,
    stream: usize,
}
impl Write for Sink {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let n = bytes.len().min(8192);
        self.sender
            .send(Output::Bytes(self.stream, bytes[..n].to_vec()))
            .map_err(|_| io::Error::from(io::ErrorKind::BrokenPipe))?;
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
impl Drop for Sink {
    fn drop(&mut self) {
        let _ = self.sender.send(Output::End(self.stream));
    }
}
pub(super) struct Ui {
    mode: Option<terminal_mode::Mode>,
    sender: mpsc::SyncSender<Output>,
    output: mpsc::Receiver<Output>,
    input: mpsc::Receiver<RawInput>,
    pending: VecDeque<Input>,
    closed: bool,
    keys: Keys,
    editor: Editor,
    utf8: [Vec<u8>; 2],
    markdown: bool,
    markdown_fence: Option<String>,
    markdown_line: Vec<u8>,
    markdown_style: markdown::Style,
    tail: String,
    drawn: String,
    cursor: usize,
    columns: usize,
    menu_rows: usize,
    images: Vec<(String, Vec<u8>)>,
}
impl Ui {
    pub fn new() -> io::Result<Self> {
        let tty = io::stdin().is_terminal()
            && io::stdout().is_terminal()
            && io::stderr().is_terminal()
            && std::env::var("TERM").as_deref() != Ok("dumb");
        let mode = if tty { Some(terminal_mode::Mode::enter()?) } else { None };
        let raw = mode.is_some();
        let (sender, output) = mpsc::sync_channel(128);
        let (tx, input) = mpsc::sync_channel(64);
        let ui = Self {
            mode,
            sender,
            output,
            input,
            pending: VecDeque::new(),
            closed: false,
            keys: Keys::default(),
            editor: Editor::default(),
            utf8: [Vec::new(), Vec::new()],
            markdown: std::env::var_os("NO_COLOR").is_none() && std::env::var("TERM").as_deref() != Ok("dumb"),
            markdown_fence: None,
            markdown_line: Vec::new(),
            markdown_style: markdown::Style { color: true },
            tail: String::new(),
            drawn: String::new(),
            cursor: 0,
            columns: 80,
            menu_rows: 0,
            images: Vec::new(),
        };
        if raw {
            io::stderr().write_all(b"\x1b[?2004h")?;
            io::stderr().flush()?;
        }
        thread::spawn(move || {
            if raw {
                let mut reader = io::stdin().lock();
                let mut bytes = [0; 1024];
                loop {
                    match reader.read(&mut bytes) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            if tx.send(RawInput::Bytes(bytes[..n].to_vec())).is_err() {
                                return;
                            }
                        }
                    }
                }
            } else {
                for line in io::stdin().lock().lines() {
                    match line {
                        Ok(line) => {
                            if tx.send(RawInput::Line(line)).is_err() {
                                return;
                            }
                        }
                        Err(_) => break,
                    }
                }
            }
            let _ = tx.send(RawInput::Eof);
        });
        Ok(ui)
    }
    pub fn sink(&self, stderr: bool) -> Sink {
        Sink { sender: self.sender.clone(), stream: usize::from(stderr) }
    }
    pub fn approval_pending(&mut self, pending: bool) -> io::Result<()> {
        if self.editor.approval_pending != pending {
            self.clear()?;
            self.editor.approval_pending = pending;
            self.draw()?;
        }
        Ok(())
    }
    fn clear(&mut self) -> io::Result<()> {
        if self.mode.is_none() {
            return Ok(());
        }
        let (columns, rows) = terminal_mode::size();
        if self.menu_rows > 0 {
            io::stderr().write_all(b"\r\x1b[0J")?;
            self.menu_rows = 0;
        }
        if !self.drawn.is_empty() && columns != self.columns {
            let up = reflow_rows(&self.drawn, self.cursor, columns).min(rows.saturating_sub(1));
            if up > 0 {
                write!(io::stderr(), "\r\x1b[{up}A")?;
            }
            io::stderr().write_all(b"\r\x1b[0J")?;
        } else {
            io::stderr().write_all(b"\r\x1b[2K")?;
        }
        self.drawn.clear();
        Ok(())
    }
    fn draw(&mut self) -> io::Result<()> {
        if self.mode.is_none() {
            return Ok(());
        }
        self.columns = terminal_mode::size().0.clamp(1, 4096);
        (self.drawn, self.cursor) = view(&self.editor, &self.tail, self.columns);
        write!(io::stderr(), "\r{}", self.drawn)?;
        let suggestions = self.editor.suggestions();
        let selected = self.editor.completion_index();
        let start = selected.saturating_sub(7);
        let max_rows = terminal_mode::size().1.saturating_sub(2).min(8);
        for (index, (name, description)) in suggestions.iter().enumerate().skip(start).take(max_rows) {
            let label = format!("{} {name}  {description}", if index == selected { ">" } else { " " });
            let mut width = 0;
            let label: String = label
                .chars()
                .take_while(|c| {
                    width += glyph(*c).1;
                    width < self.columns
                })
                .collect();
            write!(io::stderr(), "\r\n{label}")?;
            self.menu_rows += 1;
        }
        if self.menu_rows > 0 {
            write!(io::stderr(), "\x1b[{}A", self.menu_rows)?;
        }
        write!(io::stderr(), "\x1b[{}G", self.cursor + 1)?;
        io::stderr().flush()
    }
    fn finish_tail(&mut self) -> io::Result<()> {
        if !self.tail.is_empty() {
            write!(io::stderr(), "{}\r\n", self.tail)?;
            self.tail.clear();
        }
        Ok(())
    }
    fn text(&mut self, text: &str) -> io::Result<()> {
        for c in text.chars() {
            match c {
                '\n' => {
                    write!(io::stderr(), "{}\r\n", self.tail)?;
                    self.tail.clear();
                }
                '\r' => {}
                '\t' => self.tail.push_str("    "),
                '\x1b' => self.tail.push('\x1b'),
                c if c.is_control() => self.tail.push_str(&format!("\\u{{{:x}}}", c as u32)),
                c => self.tail.push(c),
            }
            if self.tail.len() >= 8192 {
                self.finish_tail()?;
            }
        }
        Ok(())
    }
    /// stdout is assistant text: buffer complete lines and render them as Markdown.
    fn write_markdown(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.markdown_line.extend_from_slice(bytes);
        while let Some(index) = self.markdown_line.iter().position(|byte| *byte == b'\n') {
            let line = String::from_utf8_lossy(&self.markdown_line[..index]).into_owned();
            self.markdown_line.drain(..=index);
            let rendered = markdown::render_line(
                line.strip_suffix('\r').unwrap_or(&line),
                &mut self.markdown_fence,
                self.markdown_style,
            );
            if self.mode.is_some() {
                self.text(&rendered)?;
                self.finish_tail()?;
            } else {
                writeln!(io::stdout(), "{rendered}")?;
            }
        }
        if self.markdown_line.len() > 1_000_000 {
            self.flush_markdown_line()?;
        }
        Ok(())
    }
    fn flush_markdown_line(&mut self) -> io::Result<()> {
        if self.markdown_line.is_empty() {
            return Ok(());
        }
        let line = String::from_utf8_lossy(&self.markdown_line).into_owned();
        self.markdown_line.clear();
        let rendered = markdown::render_line(&line, &mut self.markdown_fence, self.markdown_style);
        if self.mode.is_some() {
            self.text(&rendered)?;
            self.finish_tail()?;
        } else {
            writeln!(io::stdout(), "{rendered}")?;
        }
        Ok(())
    }
    fn decode(&mut self, stream: usize, bytes: &[u8], end: bool) -> io::Result<()> {
        if stream == 0 && self.markdown {
            self.utf8[0].extend_from_slice(bytes);
            let (valid, skip) = match std::str::from_utf8(&self.utf8[0]) {
                Ok(_) => (self.utf8[0].len(), 0),
                Err(error) => (
                    error.valid_up_to(),
                    error.error_len().unwrap_or(if end { self.utf8[0].len() - error.valid_up_to() } else { 0 }),
                ),
            };
            let text = String::from_utf8_lossy(&self.utf8[0][..valid]).replace('\x1b', "\\u{1b}");
            self.utf8[0].drain(..valid + skip);
            return self.write_markdown(text.as_bytes());
        }
        self.utf8[stream].extend_from_slice(bytes);
        loop {
            let (valid, skip) = match std::str::from_utf8(&self.utf8[stream]) {
                Ok(_) => (self.utf8[stream].len(), 0),
                Err(error) => (
                    error.valid_up_to(),
                    error.error_len().unwrap_or(if end { self.utf8[stream].len() - error.valid_up_to() } else { 0 }),
                ),
            };
            let text = String::from_utf8_lossy(&self.utf8[stream][..valid]).replace('\x1b', "\\u{1b}");
            self.text(&text)?;
            if skip > 0 {
                self.text("\u{fffd}")?;
            }
            self.utf8[stream].drain(..valid + skip);
            if skip == 0 {
                break;
            }
        }
        Ok(())
    }
    pub fn drain(&mut self) -> io::Result<()> {
        let events: Vec<_> = self.output.try_iter().take(128).collect();
        if events.is_empty() {
            return Ok(());
        }
        self.clear()?;
        for event in events {
            match event {
                Output::Bytes(stream, bytes) if self.mode.is_some() => self.decode(stream, &bytes, false)?,
                Output::End(stream) if self.mode.is_some() => {
                    self.decode(stream, &[], true)?;
                    self.finish_tail()?;
                    if stream == 0 && self.markdown {
                        self.flush_markdown_line()?;
                    }
                }
                Output::Bytes(0, bytes) => {
                    io::stdout().write_all(&bytes)?;
                    io::stdout().flush()?;
                }
                Output::Bytes(_, bytes) => {
                    io::stderr().write_all(&bytes)?;
                    io::stderr().flush()?;
                }
                _ => {}
            }
        }
        self.draw()
    }
    pub fn note(&mut self, text: &str) -> io::Result<()> {
        if self.mode.is_none() {
            return writeln!(io::stderr(), "{text}");
        }
        self.clear()?;
        self.finish_tail()?;
        self.text(text)?;
        if !text.ends_with('\n') {
            self.text("\n")?;
        }
        self.draw()
    }
    pub fn next(&mut self, timeout: Duration) -> io::Result<Option<Input>> {
        if let Some(input) = self.pending.pop_front() {
            return Ok(Some(input));
        }
        if self.closed {
            thread::sleep(timeout);
            return Ok(None);
        }
        let input = match self.input.recv_timeout(timeout) {
            Ok(input) => input,
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                self.closed = true;
                return Ok(Some(Input::Eof));
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if self.keys.expire_escape() {
                    self.clear()?;
                    self.editor.completion_key(&Key::Escape);
                    self.draw()?;
                }
                if self.mode.is_some() && terminal_mode::size().0.clamp(1, 4096) != self.columns {
                    self.clear()?;
                    self.draw()?;
                }
                return Ok(None);
            }
        };
        match input {
            RawInput::Line(line) => return Ok(Some(Input::Line(line))),
            RawInput::Eof => {
                self.closed = true;
                return Ok(Some(Input::Eof));
            }
            RawInput::Bytes(bytes) => {
                let keys = self.keys.feed(&bytes);
                if keys.is_empty() {
                    return Ok(None);
                }
                self.clear()?;
                for key in keys {
                    if matches!(key, Key::Backspace | Key::Delete)
                        && let Some((start, end)) = self
                            .images
                            .iter()
                            .filter_map(|(token, _)| {
                                self.editor.text.match_indices(token).find_map(|(start, _)| {
                                    let end = start + token.len();
                                    ((matches!(key, Key::Backspace) && end == self.editor.cursor)
                                        || (matches!(key, Key::Delete) && start == self.editor.cursor))
                                        .then_some((start, end))
                                })
                            })
                            .next()
                    {
                        self.editor.text.replace_range(start..end, "");
                        self.editor.cursor = start;
                        continue;
                    }
                    if self.editor.completion_key(&key) {
                        continue;
                    }
                    match key {
                        Key::Cancel => {
                            self.pending.clear();
                            self.pending.push_back(Input::Cancel);
                            break;
                        }
                        Key::Eof if self.editor.text.is_empty() => {
                            self.closed = true;
                            self.pending.push_back(Input::Eof);
                            break;
                        }
                        Key::Enter => {
                            let visible = self.editor.text.clone();
                            let tokens = self
                                .images
                                .iter()
                                .filter(|(token, _)| visible.contains(token))
                                .map(|(token, _)| token.clone())
                                .collect::<Vec<_>>();
                            let caption = self.editor.submit_without(&tokens);
                            self.finish_tail()?;
                            self.text(&format!("lninl> {visible}\n"))?;
                            let mut images = Vec::new();
                            for (token, bytes) in self.images.drain(..) {
                                if visible.contains(&token) {
                                    images.push(bytes);
                                }
                            }
                            self.pending.push_back(if images.is_empty() {
                                Input::Line(caption)
                            } else {
                                Input::Attachments { line: caption, images }
                            });
                        }
                        Key::Clipboard => self.pending.push_back(Input::Clipboard),
                        Key::TooLong => {
                            self.finish_tail()?;
                            self.text("[input exceeds 64 KiB; paste rejected]\n")?;
                        }
                        Key::Text(text) => {
                            if !self.editor.insert(&text) {
                                self.finish_tail()?;
                                self.text("[input exceeds 64 KiB; insertion rejected]\n")?;
                            }
                        }
                        Key::Paste(text) => {
                            if !self.editor.paste(&text) {
                                self.finish_tail()?;
                                self.text("[input exceeds 64 KiB; paste rejected]\n")?;
                            }
                        }
                        key => self.editor.edit(key),
                    }
                }
                self.draw()?;
            }
        }
        Ok(self.pending.pop_front())
    }
    pub fn stage_image(&mut self, bytes: Vec<u8>) -> io::Result<()> {
        self.images.retain(|(token, _)| self.editor.text.contains(token));
        if !bytes.starts_with(&[137, 80, 78, 71, 13, 10, 26, 10])
            || bytes.len() > 8 * 1024 * 1024
            || self.images.len() >= 8
            || self.images.iter().map(|(_, b)| b.len()).sum::<usize>() + bytes.len() > 16 * 1024 * 1024
        {
            return Err(io::Error::other("Invalid PNG or image size/count limit exceeded"));
        }
        let n = self
            .images
            .iter()
            .filter_map(|(t, _)| t.strip_prefix("[Image #")?.strip_suffix(']')?.parse::<usize>().ok())
            .max()
            .unwrap_or(0)
            + 1;
        let token = format!("[Image #{n}]");
        if !self.editor.insert(&token) {
            return Err(io::Error::other("Input exceeds 64 KiB"));
        }
        self.images.push((token, bytes));
        self.draw()
    }
}
impl Drop for Ui {
    fn drop(&mut self) {
        if self.mode.is_some() {
            let _ = self.clear();
            let _ = self.finish_tail();
            let _ = io::stderr().write_all(b"\x1b[?2004l\r\n");
            let _ = io::stderr().flush();
            // The saved terminal modes are restored by Mode::drop even on ordinary error paths.
        }
    }
}
fn reflow_rows(text: &str, cursor: usize, columns: usize) -> usize {
    let (mut row, mut column, mut cells) = (0, 0, 0);
    for c in text.chars() {
        if cells >= cursor {
            break;
        }
        let width = glyph(c).1;
        if column + width > columns {
            row += 1;
            column = 0;
        }
        column += width;
        cells += width;
        if column >= columns {
            row += 1;
            column = 0;
        }
    }
    row
}
#[cfg(test)]
#[path = "editor_tests.rs"]
mod tests;

//! Line-oriented Markdown-to-ANSI renderer for console output.
//!
//! Deliberately small: it formats headings, emphasis, bullets, quotes, rules,
//! tables and fenced code. It never rewrites paragraph text, so identifiers and
//! paths survive unchanged. When color is disabled every function returns plain
//! text with the original characters intact.

#[derive(Clone, Copy, Default)]
pub(crate) struct Style {
    pub color: bool,
}

fn paint(text: &str, code: &str, color: bool) -> String {
    if color { format!("\x1b[{code}m{text}\x1b[0m") } else { text.to_owned() }
}

/// Inline emphasis: **bold**, __bold__, *italic*, `code`.
/// Unknown markers are left alone; backtick spans are never emphasised.
pub(crate) fn inline(text: &str, style: Style) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(index) = rest.find('`') {
        out.push_str(&emphasis(&rest[..index], style));
        let tail = &rest[index + 1..];
        match tail.find('`') {
            Some(end) => {
                out.push_str(&paint(&tail[..end], "36", style.color));
                rest = &tail[end + 1..];
            }
            None => {
                out.push('`');
                rest = tail;
            }
        }
    }
    out.push_str(&emphasis(rest, style));
    out
}

fn emphasis(text: &str, style: Style) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while !rest.is_empty() {
        let bold = rest.find("**").or_else(|| rest.find("__"));
        let Some(start) = bold else { break };
        let marker = &rest[start..start + 2];
        let Some(end) = rest[start + 2..].find(marker).map(|end| start + 2 + end) else { break };
        out.push_str(&rest[..start]);
        let body = &rest[start + 2..end];
        out.push_str(&paint(&emphasis_single(body, style), "1", style.color));
        rest = &rest[end + 2..];
    }
    out.push_str(&emphasis_single(rest, style));
    out
}

fn emphasis_single(text: &str, style: Style) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find('*').filter(|i| *i < rest.len().saturating_sub(1)) {
        let Some(end) = rest[start + 1..].find('*').map(|end| start + 1 + end).filter(|end| *end > start + 1) else {
            break;
        };
        out.push_str(&rest[..start]);
        out.push_str(&paint(&rest[start + 1..end], "3", style.color));
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    out
}

fn heading(line: &str, style: Style) -> Option<String> {
    let level = line.bytes().take_while(|byte| *byte == b'#').count();
    if level == 0 || level > 6 {
        return None;
    }
    let text = line[level..].strip_prefix(' ')?;
    let code = match level {
        1 => "1;36",
        2 => "1;35",
        3 => "1;34",
        _ => "1",
    };
    if style.color {
        Some(paint(&inline(text, style), code, true))
    } else {
        Some(format!("{} {}", "#".repeat(level), inline(text, style)))
    }
}

fn fence_open(line: &str) -> Option<(&str, usize)> {
    let trimmed = line.trim_start();
    let ticks = trimmed.bytes().take_while(|byte| *byte == b'`').count();
    (ticks >= 3).then(|| (&trimmed[ticks..], ticks))
}

fn rule(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.len() >= 3
        && trimmed.chars().next().is_some_and(|first| trimmed.chars().all(|c| c == first))
        && matches!(trimmed.chars().next(), Some('-' | '*' | '_'))
}

fn bullet(trimmed: &str) -> Option<(usize, &str)> {
    let indent = trimmed.len() - trimmed.trim_start().len();
    let body = trimmed.trim_start();
    let (marker, rest) = body.split_at_checked(1)?;
    if !matches!(marker, "-" | "*" | "+") {
        return None;
    }
    Some((indent, rest.strip_prefix(' ')?))
}

fn table_row(line: &str) -> Option<Vec<&str>> {
    let trimmed = line.trim();
    if !trimmed.starts_with('|') || !trimmed.ends_with('|') || trimmed.len() < 2 {
        return None;
    }
    Some(trimmed[1..trimmed.len() - 1].split('|').map(str::trim).collect())
}

/// True when the row is the `|---|:--:|` separator of a table.
fn table_rule(cells: &[&str]) -> bool {
    !cells.is_empty()
        && cells.iter().all(|cell| {
            let cell = cell.trim_start_matches(':').trim_end_matches(':');
            !cell.is_empty() && cell.chars().all(|c| c == '-')
        })
}

fn render_row(cells: &[&str], style: Style, header: bool) -> String {
    let text = cells.iter().map(|cell| inline(cell, style)).collect::<Vec<_>>().join(" │ ");
    if header { paint(&text, "1", style.color) } else { text }
}

/// Render one line. `fence` carries the open code-fence marker across calls.
pub(crate) fn render_line(line: &str, fence: &mut Option<String>, style: Style) -> String {
    if let Some(marker) = fence.clone() {
        if line.trim_start().starts_with(&marker) {
            *fence = None;
            return paint("└─", "2", style.color);
        }
        return if style.color { format!("\x1b[2m│ {line}\x1b[0m") } else { format!("  {line}") };
    }
    if let Some((info, ticks)) = fence_open(line) {
        *fence = Some("`".repeat(ticks));
        return paint(&format!("┌─{}", info.trim()), "2", style.color);
    }
    if let Some(heading) = heading(line, style) {
        return heading;
    }
    if rule(line) {
        return paint(&"─".repeat(40), "2", style.color);
    }
    let trimmed = line.trim_start();
    if let Some(rest) = trimmed.strip_prefix('>') {
        let rest = rest.strip_prefix(' ').unwrap_or(rest);
        return paint(&format!("│ {}", inline(rest, style)), "2", style.color);
    }
    if let Some((indent, text)) = bullet(trimmed) {
        let marker = if style.color { "•" } else { "-" };
        return format!("{}{} {}", " ".repeat(indent), marker, inline(text, style));
    }
    if let Some(cells) = table_row(line) {
        if table_rule(&cells) {
            return String::new();
        }
        return render_row(&cells, style, false);
    }
    inline(line, style)
}

/// Render a complete block, used by tests and non-streaming callers.
#[cfg(test)]
pub(crate) fn render(text: &str, style: Style) -> String {
    let mut fence = None;
    let mut out: Vec<String> = Vec::new();
    let mut table: Vec<Vec<&str>> = Vec::new();
    for line in text.lines() {
        if fence.is_none()
            && let Some(cells) = table_row(line)
        {
            table.push(cells);
            continue;
        }
        if !table.is_empty() {
            out.extend(flush_table(&table, style));
            table.clear();
        }
        out.push(render_line(line, &mut fence, style));
    }
    out.extend(flush_table(&table, style));
    out.join("\n")
}

/// Tables need a lookahead pass: the row after the header decides the styling.
#[cfg(test)]
fn flush_table(rows: &[Vec<&str>], style: Style) -> Vec<String> {
    let has_rule = rows.get(1).is_some_and(|cells| table_rule(cells));
    rows.iter()
        .enumerate()
        .filter(|(index, cells)| !(*index == 1 && has_rule && table_rule(cells)))
        .map(|(index, cells)| render_row(cells, style, has_rule && index == 0))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLAIN: Style = Style { color: false };

    fn colored() -> Style {
        Style { color: true }
    }

    #[test]
    fn headings_lose_hashes_only_when_colored() {
        assert_eq!(render("# Title", PLAIN), "# Title");
        assert_eq!(render_line("# Title", &mut None, colored()), "\x1b[1;36mTitle\x1b[0m");
        assert_eq!(render("#### Deep", PLAIN), "#### Deep");
    }

    #[test]
    fn inline_code_and_bold_render_and_degrade_cleanly() {
        assert_eq!(inline("use `edit` now", PLAIN), "use edit now");
        assert_eq!(inline("use `edit` now", colored()), "use \x1b[36medit\x1b[0m now");
        assert_eq!(inline("**bold** text", PLAIN), "bold text");
        assert_eq!(inline("**bold**", colored()), "\x1b[1mbold\x1b[0m");
    }

    #[test]
    fn plain_output_has_no_escape_sequences() {
        let document = "# H\n\n**bold** and `code`\n\n```sh\nls -la\n```\n\n| a |\n|---|\n| 1 |\n";
        assert!(!render(document, PLAIN).contains('\x1b'));
    }

    #[test]
    fn fenced_blocks_are_framed_and_closed() {
        let mut fence = None;
        assert_eq!(render_line("```rust", &mut fence, PLAIN), "┌─rust");
        assert_eq!(render_line("let x = 1;", &mut fence, PLAIN), "  let x = 1;");
        assert!(fence.is_some());
        render_line("```", &mut fence, PLAIN);
        assert!(fence.is_none());
    }

    #[test]
    fn bullets_quotes_and_rules() {
        assert_eq!(render("- item", PLAIN), "- item");
        assert_eq!(render_line("- item", &mut None, colored()), "• item");
        assert_eq!(render_line("> note", &mut None, PLAIN), "│ note");
        assert_eq!(render_line("---", &mut None, PLAIN), "─".repeat(40));
    }

    #[test]
    fn tables_bold_the_header_row_only() {
        assert_eq!(render("| a | b |\n|---|---|\n| 1 | 2 |", PLAIN), "a │ b\n1 │ 2");
        let output = render("| a | b |\n|---|---|\n| 1 | 2 |", colored());
        assert!(output.starts_with("\x1b[1ma │ b\x1b[0m"));
        assert!(!output.contains("--"));
    }

    #[test]
    fn code_spans_are_not_parsed_for_emphasis() {
        assert_eq!(inline("`a_b_c`", colored()), "\x1b[36ma_b_c\x1b[0m");
        assert_eq!(inline("unclosed `tick", PLAIN), "unclosed `tick");
    }
}

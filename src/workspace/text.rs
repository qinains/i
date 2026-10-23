use anyhow::{Result, bail, ensure};

pub fn clip(text: &str, limit: usize) -> String {
    if text.len() <= limit {
        return text.into();
    }
    format!("{}\n[truncated]", &text[..text.floor_char_boundary(limit)])
}

pub fn replace_once(text: &str, old: &str, new: &str, line_number: Option<usize>) -> Result<String> {
    ensure!(!old.is_empty(), "old text cannot be empty");
    if let Some(line) = line_number {
        let lines: Vec<&str> = text.lines().collect();
        ensure!(line >= 1 && line <= lines.len(), "line_number out of range");
        let target = lines[line - 1];
        ensure!(
            target.contains(old) && target.find(old) == target.rfind(old),
            "old text not found exactly once on line"
        );
        let start = text.split_inclusive('\n').take(line - 1).map(str::len).sum::<usize>() + target.find(old).unwrap();
        let mut result = text.to_owned();
        result.replace_range(start..start + old.len(), new);
        return Ok(result);
    }
    ensure!(
        text.find(old).is_some() && text.find(old) == text.rfind(old),
        "old text must match exactly once; provide line_number"
    );
    Ok(text.replacen(old, new, 1))
}

pub fn image_mime(bytes: &[u8]) -> Result<&'static str> {
    match bytes {
        [0x89, b'P', b'N', b'G', 13, 10, 0x1a, 10, ..] => Ok("image/png"),
        [0xff, 0xd8, 0xff, ..] => Ok("image/jpeg"),
        [b'G', b'I', b'F', b'8', b'7' | b'9', b'a', ..] => Ok("image/gif"),
        [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P', ..] => Ok("image/webp"),
        _ => bail!("unsupported image format"),
    }
}

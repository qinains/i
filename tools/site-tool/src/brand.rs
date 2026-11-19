use crate::write;
use anyhow::{Result, ensure};
use flate2::{Compression, write::ZlibEncoder};
use std::{fs, io::Write as _, path::Path};
#[path = "../../../assets/brand.rs"]
mod design;
use design::{ACCENT, BACKGROUND, CELL, DOT_RADIUS, RADIUS, Shape, drawn, glyph, hex, wordmark_svg};

const VIEW: u32 = 64;
fn icon_shapes() -> impl Iterator<Item = Shape> {
    glyph(design::I, 22.0, 7.0)
}

pub fn svg() -> String {
    let background = hex(BACKGROUND);
    let mut output = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {VIEW} {VIEW}\">\n  <rect width=\"{VIEW}\" height=\"{VIEW}\" fill=\"{background}\"/>\n"
    );
    drawn(&mut output, icon_shapes(), ACCENT);
    output.push_str("</svg>\n");
    output
}

fn contains_mark(shapes: &[Shape], x: f64, y: f64) -> bool {
    let half = CELL / 2.0;
    shapes.iter().any(|shape| match *shape {
        Shape::Dot(cx, cy) => (x - cx).powi(2) + (y - cy).powi(2) < DOT_RADIUS.powi(2),
        Shape::Block(left, top) => {
            let dx = (x - left - half).abs();
            let dy = (y - top - half).abs();
            dx < half
                && dy < half
                && (dx - half + RADIUS).max(0.0).powi(2) + (dy - half + RADIUS).max(0.0).powi(2) <= RADIUS.powi(2)
        }
    })
}

fn chunk(png: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    png.extend((data.len() as u32).to_be_bytes());
    png.extend(kind);
    png.extend(data);
    let mut crc = crc32fast::Hasher::new();
    crc.update(kind);
    crc.update(data);
    png.extend(crc.finalize().to_be_bytes());
}

pub fn png(size: u32) -> Result<Vec<u8>> {
    ensure!((16..=512).contains(&size), "invalid brand icon size");
    let scale = f64::from(VIEW) / f64::from(size);
    let shapes = icon_shapes().collect::<Vec<_>>();
    let mut rows = Vec::with_capacity((size * (1 + size * 3)) as usize);
    for y in 0..size {
        rows.push(0);
        for x in 0..size {
            let mut coverage = 0;
            for dy in [0.125, 0.375, 0.625, 0.875] {
                for dx in [0.125, 0.375, 0.625, 0.875] {
                    coverage +=
                        u16::from(contains_mark(&shapes, (f64::from(x) + dx) * scale, (f64::from(y) + dy) * scale));
                }
            }
            for (bg, ink) in BACKGROUND.into_iter().zip(ACCENT) {
                rows.push(((u16::from(bg) * (16 - coverage) + u16::from(ink) * coverage + 8) / 16) as u8);
            }
        }
    }
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(&rows)?;
    let mut header = size.to_be_bytes().to_vec();
    header.extend(size.to_be_bytes());
    header.extend([8, 2, 0, 0, 0]);
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    chunk(&mut png, b"IHDR", &header);
    chunk(&mut png, b"IDAT", &encoder.finish()?);
    chunk(&mut png, b"IEND", &[]);
    Ok(png)
}

pub fn sync_svg(root: &Path, check: bool) -> Result<()> {
    let square = svg();
    let wordmark = wordmark_svg(false);
    let cover = wordmark_svg(true);
    for (path, expected) in [
        ("site/assets/favicon.svg", &square),
        ("web/icons/icon.svg", &square),
        ("site/assets/wordmark.svg", &wordmark),
        ("web/icons/wordmark.svg", &wordmark),
        ("assets/lninl.svg", &cover),
    ] {
        let path = root.join(path);
        if check {
            ensure!(fs::read(&path)? == expected.as_bytes(), "Stale icon: {}", path.display());
        } else {
            write(&path, expected)?;
        }
    }
    Ok(())
}

pub fn build_pwa(root: &Path) -> Result<()> {
    sync_svg(root, true)?;
    for size in [192, 512] {
        write(&root.join(format!("web/icons/icon-{size}.png")), png(size)?)?;
    }
    Ok(())
}

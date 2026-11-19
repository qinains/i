use std::fmt::Write as _;

pub const BACKGROUND: [u8; 3] = [15, 23, 42];
pub const ACCENT: [u8; 3] = [34, 211, 238];
const NEUTRAL: [u8; 3] = [203, 213, 225];
pub const CELL: f64 = 6.0;
pub const RADIUS: f64 = 0.875;
pub const DOT_RADIUS: f64 = 3.5;
const STEP: f64 = 7.0;
pub const L: [&str; 7] = ["## ", " # ", " # ", " # ", " # ", " # ", " ##"];
pub const N: [&str; 7] = ["     ", "     ", "# ## ", "##  #", "#   #", "#   #", "#   #"];
pub const I: [&str; 7] = [" o ", "   ", " # ", " # ", " # ", " # ", " # "];
pub const WORD: [[&str; 7]; 5] = [L, N, I, N, L];

pub enum Shape {
    Block(f64, f64),
    Dot(f64, f64),
}

pub fn glyph(rows: [&'static str; 7], left: f64, top: f64) -> impl Iterator<Item = Shape> {
    rows.into_iter().enumerate().flat_map(move |(row, text)| {
        text.bytes().enumerate().filter_map(move |(column, pixel)| {
            let x = left + column as f64 * STEP;
            let y = top + row as f64 * STEP;
            match pixel {
                b'#' => Some(Shape::Block(x, y)),
                b'o' => Some(Shape::Dot(x + CELL / 2.0, y + 4.0)),
                _ => None,
            }
        })
    })
}

pub fn hex([r, g, b]: [u8; 3]) -> String {
    format!("#{r:02x}{g:02x}{b:02x}")
}

pub fn drawn(output: &mut String, shapes: impl Iterator<Item = Shape>, color: [u8; 3]) {
    writeln!(output, "  <g fill=\"{}\">", hex(color)).unwrap();
    for shape in shapes {
        match shape {
            Shape::Block(x, y) => {
                writeln!(output, "    <rect x=\"{x}\" y=\"{y}\" width=\"{CELL}\" height=\"{CELL}\" rx=\"{RADIUS}\"/>")
                    .unwrap()
            }
            Shape::Dot(x, y) => writeln!(output, "    <circle cx=\"{x}\" cy=\"{y}\" r=\"{DOT_RADIUS}\"/>").unwrap(),
        }
    }
    output.push_str("  </g>\n");
}

pub fn wordmark_svg(cover: bool) -> String {
    let (width, height, pad, top, radius) = if cover { (228, 104, 20., 15., 12) } else { (204, 62, 8., 7., 7) };
    let bg = hex(BACKGROUND);
    let mut output = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {width} {height}\" role=\"img\" aria-labelledby=\"title\">\n  <title id=\"title\">lninl</title>\n  <rect width=\"{width}\" height=\"{height}\" rx=\"{radius}\" fill=\"{bg}\"/>\n"
    );
    let mut x = pad;
    for (column, rows) in WORD.iter().enumerate() {
        drawn(&mut output, glyph(*rows, x, top), if column == 2 { ACCENT } else { NEUTRAL });
        x += (rows[0].len() + 2) as f64 * STEP;
    }
    if cover {
        output.push_str("  <text x=\"114\" y=\"87\" text-anchor=\"middle\" font-family=\"monospace\" font-size=\"13\" fill=\"#e2e8f0\">lninl / i</text>\n");
    }
    output.push_str("</svg>\n");
    output
}

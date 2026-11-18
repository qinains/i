//! Optional presentation plugin. Never included in the model prompt or core budget.
const L: [&str; 7] = [" ##  ", "  #  ", "  #  ", "  #  ", "  #  ", "  #  ", "  ## "];
const N: [&str; 7] = ["     ", "     ", "# ## ", "##  #", "#   #", "#   #", "#   #"];
const I: [&str; 7] = ["  o  ", "     ", "  #  ", "  #  ", "  #  ", "  #  ", " ### "];

fn glyph_row(column: usize, row: usize) -> String {
    let text = [L, N, I, N, L][column][row];
    if column > 2 { text.chars().rev().collect() } else { text.into() }
}

pub(crate) fn render(color: bool, ascii: bool) -> String {
    let mut output = String::from("\n");
    for row in 0..7 {
        output.push_str("  ");
        for column in 0..5 {
            if color {
                output.push_str(if column == 2 { "\x1b[1;36m" } else { "\x1b[37m" });
            }
            let text = glyph_row(column, row);
            output.push_str(&if ascii { text } else { text.replace('#', "█").replace('o', "●") });
            if color {
                output.push_str("\x1b[0m");
            }
            output.push_str("  ");
        }
        output.push('\n');
    }
    output.push_str("\n  lninl / i\n  Small core. Your tools. Your model.\n\n");
    output
}
pub(crate) fn svg() -> String {
    use std::fmt::Write;
    let mut output = String::from(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 343 132" role="img" aria-labelledby="title"><title id="title">lninl — i</title><rect width="343" height="132" rx="16" fill="#0f172a"/>"##,
    );
    for column in 0..5 {
        let color = if column == 2 { "#22d3ee" } else { "#94a3b8" };
        for row in 0..7 {
            for (cell, pixel) in glyph_row(column, row).chars().enumerate() {
                let (x, y) = (24 + column * 64 + cell * 8, 20 + row * 8);
                if pixel == '#' {
                    write!(output, r#"<rect x="{x}" y="{y}" width="7" height="7" rx="1" fill="{color}"/>"#).unwrap();
                }
                if pixel == 'o' {
                    write!(output, r#"<circle cx="{}" cy="{}" r="4" fill="{color}"/>"#, x as f64 + 3.5, y as f64 + 3.5)
                        .unwrap();
                }
            }
        }
    }
    output.push_str(r##"<text x="171.5" y="112" text-anchor="middle" font-family="monospace" font-size="13" fill="#e2e8f0">lninl / i</text></svg>"##);
    output
}
pub(crate) fn show() {
    use std::io::IsTerminal;
    eprint!("{}", render(std::io::stderr().is_terminal() && std::env::var_os("NO_COLOR").is_none(), false));
}

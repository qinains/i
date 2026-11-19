//! Optional presentation plugin. Never included in the model prompt or core budget.
#[path = "../../../assets/brand.rs"]
mod design;

pub(crate) fn render(color: bool, ascii: bool) -> String {
    let mut output = String::from("\n");
    for row in 0..7 {
        output.push_str("  ");
        for (column, glyph) in design::WORD.iter().enumerate() {
            if color {
                output.push_str(if column == 2 { "\x1b[1;36m" } else { "\x1b[37m" });
            }
            let text = glyph[row];
            if ascii {
                output.push_str(text);
            } else {
                output.push_str(&text.replace('#', "█").replace('o', "●"));
            }
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
    design::wordmark_svg(true)
}
pub(crate) fn show() {
    use std::io::IsTerminal;
    eprint!("{}", render(std::io::stderr().is_terminal() && std::env::var_os("NO_COLOR").is_none(), false));
}

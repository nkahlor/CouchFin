//! Every colour, length and font weight comes from `Theme` in `ui/theme.slint`,
//! so a theme can change the look in one place (CLAUDE.md, code conventions).
//! This scans every other `.slint` file for literals that bypass it.

use std::fs;
use std::path::{Path, PathBuf};

const THEME: &str = "theme.slint";
const UNITS: &[&str] = &["px", "phx", "pt", "in", "mm", "cm", "rem"];
const COLOUR_FUNCTIONS: &[&str] = &["rgb(", "rgba(", "hsv(", "hsva(", "Colors."];

fn slint_files(dir: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("ui directory is readable") {
        let path = entry.expect("directory entry").path();
        if path.is_dir() {
            slint_files(&path, files);
        } else if path.extension().is_some_and(|ext| ext == "slint") {
            files.push(path);
        }
    }
}

/// The line without a `//` comment or the contents of string literals, so
/// prose and text shown on screen don't count.
fn code_only(line: &str) -> String {
    let mut code = String::new();
    let mut in_string = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                in_string = !in_string;
                code.push('"');
            }
            '\\' if in_string => {
                chars.next();
            }
            '/' if !in_string && chars.peek() == Some(&'/') => break,
            _ if in_string => {}
            _ => code.push(c),
        }
    }
    code
}

/// A number with a length unit (`96px`), other than zero.
fn has_length_literal(code: &str) -> bool {
    let bytes = code.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let starts_number = bytes[i].is_ascii_digit()
            && (i == 0 || !(bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'-'));
        if !starts_number {
            i += 1;
            continue;
        }
        let start = i;
        while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
            i += 1;
        }
        let number = &code[start..i];
        let rest = &code[i..];
        let unit = UNITS.iter().find(|unit| {
            rest.starts_with(*unit)
                && !rest[unit.len()..]
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_alphanumeric() || c == '-')
        });
        if unit.is_some() && number.parse::<f64>().is_ok_and(|n| n != 0.0) {
            return true;
        }
    }
    false
}

/// `#rgb`, `#rrggbb` or `#rrggbbaa`.
fn has_hex_colour(code: &str) -> bool {
    code.match_indices('#').any(|(i, _)| {
        let hex: String = code[i + 1..]
            .chars()
            .take_while(char::is_ascii_hexdigit)
            .collect();
        matches!(hex.len(), 3 | 4 | 6 | 8)
    })
}

fn has_font_weight_literal(code: &str) -> bool {
    code.split_once("font-weight:")
        .is_some_and(|(_, value)| value.trim_start().starts_with(|c: char| c.is_ascii_digit()))
}

fn literals_outside_theme() -> Vec<String> {
    let ui = Path::new(env!("CARGO_MANIFEST_DIR")).join("ui");
    let mut files = Vec::new();
    slint_files(&ui, &mut files);
    assert!(!files.is_empty(), "found no .slint files");
    let mut found = Vec::new();
    for file in files.iter().filter(|f| !f.ends_with(THEME)) {
        let text = fs::read_to_string(file).expect("slint file is readable");
        for (n, line) in text.lines().enumerate() {
            let code = code_only(line);
            if has_length_literal(&code)
                || has_hex_colour(&code)
                || COLOUR_FUNCTIONS.iter().any(|f| code.contains(f))
                || has_font_weight_literal(&code)
            {
                let name = file.strip_prefix(&ui).unwrap_or(file).display();
                found.push(format!("{name}:{}: {}", n + 1, line.trim()));
            }
        }
    }
    found
}

#[test]
fn styling_values_come_from_the_theme() {
    let found = literals_outside_theme();
    assert!(
        found.is_empty(),
        "use a Theme property (ui/theme.slint) instead of these literals:\n{}",
        found.join("\n")
    );
}

#[test]
fn the_scanner_catches_each_kind_of_literal() {
    assert!(has_length_literal("width: 480px;"));
    assert!(has_length_literal("width: root.width - 96px;"));
    assert!(has_length_literal("font-size: 1.5rem;"));
    assert!(!has_length_literal("x: 0px;"));
    assert!(!has_length_literal("width: Theme.form-width;"));
    assert!(!has_length_literal("width: 50%;"));
    assert!(has_hex_colour("background: #101418;"));
    assert!(has_hex_colour("color: #fff;"));
    assert!(!has_hex_colour("background: Theme.surface;"));
    assert!(has_font_weight_literal("font-weight: 700;"));
    assert!(!has_font_weight_literal("font-weight: Theme.title-weight;"));
    assert_eq!(code_only(r#"text: "96px #fff"; // 480px"#), r#"text: ""; "#);
}

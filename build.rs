// SPDX-License-Identifier: MIT
//
// Build-time lint: reject raw string literals passed directly to `Text::new()`
// inside `src/menu/song_editor/`.
//
// A "raw" string literal here is one whose content contains at least one ASCII
// letter AND at least one whitespace character — a reliable fingerprint of
// natural-language text that should instead come from the localization system
// via `loc.msg("key")`.
//
// Patterns that are intentionally allowed:
//   Text::new("")           — empty placeholder
//   Text::new("&")          — single punctuation symbol
//   Text::new("↑")          — unicode arrow (no ASCII letter)
//   Text::new(some_var)     — variable (no leading `"`)
//   Text::new(format!(...)) — dynamic string (no leading `"`)
//   Text::new(String::from(loc.msg("key"))) — already localized
//
// Only `src/menu/song_editor/` is checked here; other directories still have
// raw strings to migrate and are not yet covered.

use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=src/menu/song_editor");

    let dir = Path::new("src/menu/song_editor");
    if !dir.exists() {
        return;
    }

    let mut violations: Vec<String> = Vec::new();

    for entry in std::fs::read_dir(dir).expect("song_editor dir must be readable") {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let source = std::fs::read_to_string(&path).unwrap_or_default();
        for (lineno, line) in source.lines().enumerate() {
            if is_raw_text_new(line) {
                violations.push(format!(
                    "{}:{}: Text::new(\"...\") with a natural-language literal — \
                     use loc.msg(\"key\") instead",
                    path.display(),
                    lineno + 1,
                ));
            }
        }
    }

    if !violations.is_empty() {
        eprintln!();
        eprintln!("────────────────────────────────────────────────────────────");
        eprintln!("  Localization enforcement: hardcoded user-visible strings");
        eprintln!("────────────────────────────────────────────────────────────");
        for v in &violations {
            eprintln!("  {v}");
        }
        eprintln!();
        eprintln!("  Add a key to assets/locales/en-US/main/ui.ftl and call");
        eprintln!("  loc.msg(\"your-key\") instead of the string literal.");
        eprintln!("────────────────────────────────────────────────────────────");
        eprintln!();
        std::process::exit(1);
    }
}

/// Returns `true` when `line` contains `Text::new("` where the quoted content
/// has at least one ASCII alphabetic character AND at least one whitespace
/// character — the two-feature fingerprint of natural-language text.
fn is_raw_text_new(line: &str) -> bool {
    // Trim leading whitespace and ignore comment lines.
    let trimmed = line.trim_start();
    if trimmed.starts_with("//") {
        return false;
    }

    const NEEDLE: &str = "Text::new(\"";
    let Some(pos) = line.find(NEEDLE) else { return false };
    let after_quote = &line[pos + NEEDLE.len()..];

    // Collect content up to the closing `"`, respecting `\"` escapes.
    let mut content = String::new();
    let mut chars = after_quote.chars().peekable();
    loop {
        match chars.next() {
            None | Some('"') => break,
            Some('\\') => { chars.next(); } // skip escaped character
            Some(c) => content.push(c),
        }
    }

    content.chars().any(|c| c.is_ascii_alphabetic())
        && content.chars().any(|c| c.is_ascii_whitespace())
}

#[cfg(test)]
mod tests {
    use super::is_raw_text_new;

    #[test]
    fn flags_natural_language() {
        assert!(is_raw_text_new(r#"Text::new("▶ Play")"#));
        assert!(is_raw_text_new(r#"Text::new("Another note is already here")"#));
        assert!(is_raw_text_new(r#"Text::new("✓ PERFECT G4 +10 pts")"#));
    }

    #[test]
    fn allows_empty_and_symbols() {
        assert!(!is_raw_text_new(r#"Text::new("")"#));
        assert!(!is_raw_text_new(r#"Text::new("&")"#));
        assert!(!is_raw_text_new(r#"Text::new("↑")"#));
        assert!(!is_raw_text_new(r#"Text::new("■")"#));
    }

    #[test]
    fn allows_variables_and_format() {
        assert!(!is_raw_text_new(r#"Text::new(some_var)"#));
        assert!(!is_raw_text_new(r#"Text::new(format!("{}", n))"#));
        assert!(!is_raw_text_new(r#"Text::new(String::from(label))"#));
        assert!(!is_raw_text_new(r#"Text::new(String::from(loc.msg("key")))"#));
    }

    #[test]
    fn ignores_comment_lines() {
        assert!(!is_raw_text_new(r#"// Text::new("some words here")"#));
    }
}

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

#[cfg(target_os = "windows")]
fn main() {
    build();
    generate_wix_assets().unwrap();
}

#[cfg(not(target_os = "windows"))]
fn main() {
    build();
}

fn build() {
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
    let Some(pos) = line.find(NEEDLE) else {
        return false;
    };
    let after_quote = &line[pos + NEEDLE.len()..];

    // Collect content up to the closing `"`, respecting `\"` escapes.
    let mut content = String::new();
    let mut chars = after_quote.chars().peekable();
    loop {
        match chars.next() {
            None | Some('"') => break,
            Some('\\') => {
                chars.next();
            } // skip escaped character
            Some(c) => content.push(c),
        }
    }

    content.chars().any(|c| c.is_ascii_alphabetic())
        && content.chars().any(|c| c.is_ascii_whitespace())
}

#[cfg(target_os = "windows")]
use std::io::Write;

#[cfg(target_os = "windows")]
fn generate_wix_assets() -> std::io::Result<()> {
    let assets_dir = Path::new("assets");

    if !assets_dir.exists() {
        return Ok(());
    }

    std::fs::create_dir_all("wix")?;

    let file = std::fs::File::create("wix/assets.wxi")?;
    let mut out = std::io::BufWriter::new(file);

    writeln!(
        out,
        r#"<Include> 
    <DirectoryRef Id="APPLICATIONFOLDER">
      <Directory Id="AssetsFolder" Name="assets">"#
    )?;

    let mut component_refs = Vec::new();

    visit_assets(
        assets_dir,
        assets_dir,
        &mut out,
        &mut component_refs,
    )?;

    writeln!(
        out,
        r#"
      </Directory>
    </DirectoryRef>

    <ComponentGroup Id="AssetsGroup">"#
    )?;

    for id in component_refs {
        writeln!(out, r#"      <ComponentRef Id="{id}"/>"#)?;
    }

    writeln!(
        out,
        r#"
    </ComponentGroup> 
</Include>"#
    )?;

    Ok(())
} 

#[cfg(target_os = "windows")]
fn visit_assets(
    root: &Path,
    current: &Path,
    out: &mut dyn std::io::Write,
    component_refs: &mut Vec<String>,
) -> std::io::Result<()> {
    let mut entries: Vec<_> = std::fs::read_dir(current)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();

    entries.sort();

    for path in entries {
        if path.is_dir() {
            let rel = path.strip_prefix(root).unwrap_or(&path);
            let name = path.file_name().unwrap().to_string_lossy();

            let dir_id = sanitize_wix_id(&rel.to_string_lossy());
            let dir_id = format!("A14e9533a2bd754b0bd9{}", dir_id);
            let dir_id = format!("Dir_{}", &dir_id[..12]);

            writeln!(
                out,
                r#"<Directory Id="{dir_id}" Name="{name}">"#
            )?;

            visit_assets(root, &path, out, component_refs)?;

            writeln!(out, "</Directory>")?;
        } else {
            let rel = path.strip_prefix(root).unwrap_or(&path);
            let id = sanitize_wix_id(&rel.to_string_lossy());

            let component_id = format!("Comp_{id}");
            let file_id = format!("File_{id}");

            component_refs.push(component_id.clone());

            let source = path.to_string_lossy().replace('/', "\\");

            writeln!(
                out,
                r#"
<Component Id="{component_id}" Guid="*">
    <File Id="{file_id}" Source="{source}" KeyPath="yes"/>
</Component>"#
            )?;
        }
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn sanitize_wix_id(input: &str) -> String {
    let mut s = String::new();

    for c in input.chars() {
        if c.is_ascii_alphanumeric() {
            s.push(c);
        } else {
            s.push('_');
        }
    }

    if s.is_empty() {
        return "_asset".to_string();
    }

    if !s.chars().next().unwrap().is_ascii_alphabetic() {
        s.insert(0, '_');
    }

    s
}

#[cfg(test)]
mod tests {
    use super::is_raw_text_new;

    #[test]
    fn flags_natural_language() {
        assert!(is_raw_text_new(r#"Text::new("▶ Play")"#));
        assert!(is_raw_text_new(
            r#"Text::new("Another note is already here")"#
        ));
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
        assert!(!is_raw_text_new(
            r#"Text::new(String::from(loc.msg("key")))"#
        ));
    }

    #[test]
    fn ignores_comment_lines() {
        assert!(!is_raw_text_new(r#"// Text::new("some words here")"#));
    }
}

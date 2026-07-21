// SPDX-License-Identifier: MIT
//
// Build-time lint: reject hardcoded natural-language strings reaching the
// screen, anywhere under `src/`. Four sink shapes are checked:
//
//   1. Text::new("...") / Text::from("...") — a literal passed directly.
//   2. Text::new(format!("...")) / Text::from(format!("...")) /
//      TextSpan::new(format!("...")) / TextSpan::from(format!("...")) — a
//      literal template, formatted. Only the literal *template* pieces are
//      inspected (e.g. `format!("Key: {}", key)` flags on `"Key: {}"`); the
//      format! call may itself span two lines (`format!(\n    "..."\n)`).
//   3. Text({"..."}) — the `bsn!` macro's literal-binding shape.
//   4. A literal passed as the label/text argument to one of a fixed list of
//      shared spawn helpers (`KNOWN_LABEL_SINKS`) that all display it —
//      whether on the same line as the call or, since these are often
//      multi-line calls, on one of the next few argument lines.
//
// A "raw" string literal is one whose content contains at least one ASCII
// letter AND at least one whitespace character — a reliable fingerprint of
// natural-language text that should instead come from the localization
// system via `loc.msg("key")`/`loc.msg_args("key", &[...])`. This
// deliberately does not flag single-word literals (e.g. "Retry", "Cancel")
// — widening the fingerprint itself (not just the sink shapes it's applied
// to) is a separate, much larger content-migration task; see TODO.md.
//
// Patterns that are intentionally allowed:
//   Text::new("")             — empty placeholder
//   Text::new("&")            — single punctuation symbol
//   Text::new("↑")            — unicode arrow (no ASCII letter)
//   Text::new("Retry")        — single word (no whitespace)
//   Text::new(some_var)       — variable (no leading `"`)
//   Text::new(format!("{}", n))         — no literal words in the template
//   Text::new(String::from(loc.msg("key"))) — already localized
//
// `mod tests { ... }` blocks are exempt — test fixture text is never
// user-visible. Every `mod tests` in this codebase is the last item in its
// file (convention, not enforced elsewhere), so once a `mod tests {` line is
// seen the rest of the file is skipped rather than tracking brace depth.
//
// A second, independent check below enforces that every `#[derive(Message)]`
// type is registered somewhere with `.add_message::<T>()`: Bevy 0.19 panics
// at runtime — "Message not initialized" — the moment a `MessageReader`/
// `MessageWriter` for an unregistered message type actually runs, which is
// easy to miss until that code path fires (`ExternalFolderChanged` shipped
// this way once). Same static/textual approach as the localization check:
// it can't see through a derive attribute split across multiple lines, but
// every message in this codebase is declared as a single-line
// `#[derive(..., Message, ...)]` immediately followed by its `struct`/`enum`.

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

/// Sink constructors whose argument is a literal `Text`/`TextSpan` value —
/// checked directly, and with a `format!(` wrapper (same line or the line
/// immediately after, for a `format!(\n    "..."\n)` call).
const TEXT_CTORS: &[&str] = &[
    "Text::new(",
    "Text::from(",
    "TextSpan::new(",
    "TextSpan::from(",
];

/// Shared spawn helpers whose first `&str`/`String` argument is a label/text
/// that gets displayed as-is — so a literal passed here is exactly as
/// user-visible as one passed straight to `Text::new(...)`. Each is checked
/// both for a same-line literal and, since most real calls are multi-line,
/// for a literal on one of the next few argument lines.
const KNOWN_LABEL_SINKS: &[&str] = &[
    "button::default(",
    "button::small(",
    "button::sized(",
    "spawn_button(",
    "spawn_combobox(",
    "spawn_text_row(",
    "spawn_stat_row(",
    "spawn_technique_row(",
];

/// How many lines ahead of a [`KNOWN_LABEL_SINKS`] call (or a `format!(`
/// left open at end of line) to look for its literal argument.
const LOOKAHEAD_LINES: usize = 6;

fn build() {
    println!("cargo:rerun-if-changed=src");

    let dir = Path::new("src");
    if !dir.exists() {
        return;
    }

    let mut rs_files: Vec<std::path::PathBuf> = Vec::new();
    collect_rs_files(dir, &mut rs_files);
    let sources: Vec<(String, String)> = rs_files
        .iter()
        .map(|p| {
            (
                p.display().to_string(),
                std::fs::read_to_string(p).unwrap_or_default(),
            )
        })
        .collect();

    let mut violations: Vec<String> = Vec::new();
    for (path, source) in &sources {
        check_source(source, &mut |lineno, message| {
            violations.push(format!("{path}:{}: {message}", lineno + 1));
        });
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
        eprintln!("  loc.msg(\"your-key\")/loc.msg_args(\"your-key\", &[...]) instead");
        eprintln!("  of the string literal.");
        eprintln!("────────────────────────────────────────────────────────────");
        eprintln!();
    }

    let message_violations = message_registration_violations(&sources);
    if !message_violations.is_empty() {
        eprintln!();
        eprintln!("────────────────────────────────────────────────────────────");
        eprintln!("  Message registration enforcement: unregistered #[derive(Message)]");
        eprintln!("────────────────────────────────────────────────────────────");
        for v in &message_violations {
            eprintln!("  {v}");
        }
        eprintln!();
        eprintln!("  Add `.add_message::<YourType>()` to the owning Plugin's build(),");
        eprintln!("  next to its sibling messages — otherwise the first");
        eprintln!("  MessageReader/MessageWriter for it panics at runtime instead of");
        eprintln!("  failing here at build time.");
        eprintln!("────────────────────────────────────────────────────────────");
        eprintln!();
    }

    if !violations.is_empty() || !message_violations.is_empty() {
        std::process::exit(1);
    }
}

/// Recursively collects every `.rs` file under `dir` into `out`.
fn collect_rs_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect_rs_files(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

/// Walks `source` line by line, calling `report(lineno, message)` for every
/// violation found. Split out from [`build`] so tests can exercise it
/// directly against an in-memory source string instead of real files.
fn check_source(source: &str, report: &mut dyn FnMut(usize, &str)) {
    let lines: Vec<&str> = source.lines().collect();

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        if trimmed == "mod tests {" {
            break;
        }
        if trimmed.starts_with("//") {
            continue;
        }

        // 1 & 2: Text::new/from(...) and TextSpan::new/from(...), literal or format!.
        for ctor in TEXT_CTORS {
            let needle = format!("{ctor}\"");
            if let Some(content) = extract_quoted_after(line, &needle)
                && is_natural_language(&content)
            {
                report(
                    i,
                    &format!(
                        "{ctor}\"...\") with a natural-language literal — use loc.msg(\"key\") instead"
                    ),
                );
            }

            let fmt_needle = format!("{ctor}format!(\"");
            if let Some(content) = extract_quoted_after(line, &fmt_needle)
                && is_natural_language(&content)
            {
                report(
                    i,
                    &format!(
                        "{ctor}format!(\"...\")) with a natural-language template — use loc.msg_args(\"key\", &[...]) instead"
                    ),
                );
            } else if line.trim_end().ends_with(&format!("{ctor}format!(")) {
                // `format!(` left open at end of line — the template string
                // is the first non-blank line after it.
                check_lookahead_literal(&lines, i, 1, &format!("{ctor}format!(...))"), report);
            }
        }

        // 3: bsn!'s Text({"..."}) binding shape.
        if let Some(content) = extract_quoted_after(line, "Text({\"")
            && is_natural_language(&content)
        {
            report(
                i,
                "Text({\"...\"}) with a natural-language literal — use loc.msg(\"key\") instead",
            );
        }

        // 4: known shared label-spawning helpers, same line or looked ahead.
        for sink in KNOWN_LABEL_SINKS {
            let needle = format!("{sink}\"");
            if let Some(content) = extract_quoted_after(line, &needle) {
                if is_natural_language(&content) {
                    report(
                        i,
                        &format!(
                            "{sink}\"...\") with a natural-language literal — use loc.msg(\"key\") instead"
                        ),
                    );
                }
            } else if line.contains(sink) {
                check_lookahead_literal(
                    &lines,
                    i,
                    0,
                    &format!("{sink}...) with a natural-language literal argument"),
                    report,
                );
            }
        }
    }
}

/// Every `#[derive(..., Message, ...)]` type declared in `source`, paired
/// with the derive attribute's own line number (for the report) — found by
/// looking at the first non-blank line after the attribute for a
/// `struct `/`enum ` keyword. Stops at `mod tests {` like [`check_source`]:
/// a message type declared only for a test fixture needs no production
/// registration.
fn collect_declared_messages(source: &str) -> Vec<(usize, String)> {
    let lines: Vec<&str> = source.lines().collect();
    let mut out = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed == "mod tests {" {
            break;
        }
        let Some(inner) = trimmed
            .strip_prefix("#[derive(")
            .and_then(|s| s.strip_suffix(")]"))
        else {
            continue;
        };
        if !inner.split(',').any(|d| d.trim() == "Message") {
            continue;
        }
        if let Some(name) = lines
            .iter()
            .skip(i + 1)
            .find(|l| !l.trim().is_empty())
            .and_then(|l| extract_type_name(l.trim()))
        {
            out.push((i, name));
        }
    }
    out
}

/// The identifier after a `struct `/`enum ` keyword, tolerating a leading
/// `pub`/`pub(crate)`/`pub(super)` visibility modifier.
fn extract_type_name(line: &str) -> Option<String> {
    let mut rest = line;
    if let Some(after_pub) = rest.strip_prefix("pub") {
        rest = after_pub.trim_start();
        if let Some(after_paren) = rest.strip_prefix('(') {
            let close = after_paren.find(')')?;
            rest = after_paren[close + 1..].trim_start();
        }
    }
    let rest = rest
        .strip_prefix("struct ")
        .or_else(|| rest.strip_prefix("enum "))?;
    let name: String = rest
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    (!name.is_empty()).then_some(name)
}

/// Every type named in an `add_message::<...>()` call in `source` — just the
/// final path segment, so `add_message::<pages::lessons::LessonUnitChanged>()`
/// registers as `LessonUnitChanged`, matching how [`collect_declared_messages`]
/// names a type regardless of which module declares it.
fn collect_registered_messages(source: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = source;
    while let Some(pos) = rest.find("add_message::<") {
        let after = &rest[pos + "add_message::<".len()..];
        let Some(end) = after.find('>') else { break };
        if let Some(name) = after[..end].rsplit("::").next() {
            out.push(name.trim().to_string());
        }
        rest = &after[end..];
    }
    out
}

/// Cross-file check: every declared `#[derive(Message)]` type must appear in
/// at least one `add_message::<T>()` call somewhere in `sources` (path,
/// content pairs) — see the module doc comment for why this matters.
fn message_registration_violations(sources: &[(String, String)]) -> Vec<String> {
    let mut declared: Vec<(&str, usize, String)> = Vec::new();
    let mut registered: std::collections::HashSet<String> = std::collections::HashSet::new();

    for (path, source) in sources {
        declared.extend(
            collect_declared_messages(source)
                .into_iter()
                .map(|(line, name)| (path.as_str(), line, name)),
        );
        registered.extend(collect_registered_messages(source));
    }

    declared
        .into_iter()
        .filter(|(_, _, name)| !registered.contains(name))
        .map(|(path, line, name)| {
            format!(
                "{path}:{}: `{name}` derives Message but is never registered with \
                 `.add_message::<{name}>()`",
                line + 1
            )
        })
        .collect()
}

/// Scans up to [`LOOKAHEAD_LINES`] lines starting `start_offset` lines after
/// `from`, skipping blank lines and argument lines that aren't a bare quoted
/// literal (e.g. `commands,`, `&algo_labels(),` — an identifier/expression
/// argument), until it finds the first line that *is* one (optionally
/// `&`-prefixed, optionally trailing comma) — the shape a literal takes as
/// its own argument line in a multi-line call. Reports against `from` (the
/// sink call's own line) if that literal is natural language, then stops
/// either way: only the first such argument line is ever the label in the
/// helpers this is used for.
fn check_lookahead_literal(
    lines: &[&str],
    from: usize,
    start_offset: usize,
    message: &str,
    report: &mut dyn FnMut(usize, &str),
) {
    for line in lines.iter().skip(from + start_offset).take(LOOKAHEAD_LINES) {
        let trimmed = line.trim();
        if trimmed.is_empty() || !(trimmed.starts_with('"') || trimmed.starts_with("&\"")) {
            continue;
        }
        let Some(content) = extract_quoted_after(trimmed, "\"") else {
            continue;
        };
        if is_natural_language(&content) {
            report(from, message);
        }
        return;
    }
}

/// Content between `needle` and the next unescaped `"` in `line`, or `None`
/// if `needle` doesn't occur.
fn extract_quoted_after(line: &str, needle: &str) -> Option<String> {
    let pos = line.find(needle)?;
    let after_quote = &line[pos + needle.len()..];
    let mut content = String::new();
    let mut chars = after_quote.chars().peekable();
    loop {
        match chars.next() {
            None => break,
            Some('"') => return Some(content),
            Some('\\') => {
                chars.next(); // skip escaped character
            }
            Some(c) => content.push(c),
        }
    }
    // No closing quote on this line — treat as no match rather than
    // guessing at partial content.
    None
}

/// The two-feature fingerprint of natural-language text: at least one ASCII
/// letter AND at least one ASCII whitespace character.
fn is_natural_language(content: &str) -> bool {
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

    visit_assets(assets_dir, assets_dir, &mut out, &mut component_refs)?;

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

            writeln!(out, r#"<Directory Id="{dir_id}" Name="{name}">"#)?;

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
    use super::check_source;

    fn violations(source: &str) -> Vec<String> {
        let mut out = Vec::new();
        check_source(source, &mut |lineno, message| {
            out.push(format!("{lineno}: {message}"));
        });
        out
    }

    #[test]
    fn flags_natural_language_in_text_new() {
        assert_eq!(violations(r#"Text::new("▶ Play")"#).len(), 1);
        assert_eq!(
            violations(r#"Text::new("Another note is already here")"#).len(),
            1
        );
        assert_eq!(violations(r#"Text::new("✓ PERFECT G4 +10 pts")"#).len(), 1);
    }

    #[test]
    fn allows_empty_symbols_and_single_words() {
        assert!(violations(r#"Text::new("")"#).is_empty());
        assert!(violations(r#"Text::new("&")"#).is_empty());
        assert!(violations(r#"Text::new("↑")"#).is_empty());
        assert!(violations(r#"Text::new("■")"#).is_empty());
        assert!(violations(r#"Text::new("Retry")"#).is_empty());
    }

    #[test]
    fn allows_variables_and_contentless_format() {
        assert!(violations(r#"Text::new(some_var)"#).is_empty());
        assert!(violations(r#"Text::new(format!("{}", n))"#).is_empty());
        assert!(violations(r#"Text::new(String::from(label))"#).is_empty());
        assert!(violations(r#"Text::new(String::from(loc.msg("key")))"#).is_empty());
    }

    #[test]
    fn flags_natural_language_inside_format() {
        assert_eq!(violations(r#"Text::new(format!("Key: {}", key))"#).len(), 1);
        assert_eq!(
            violations(r#"*text = Text::new(format!("Score: {}", score.points));"#).len(),
            1
        );
    }

    #[test]
    fn flags_natural_language_format_split_across_two_lines() {
        let source =
            "*text = Text::new(format!(\n    \"Play it \u{2014} target {target_note}\"\n));";
        assert_eq!(violations(source).len(), 1);
    }

    #[test]
    fn flags_natural_language_in_bsn_text_binding() {
        assert_eq!(violations(r#"Text({"Wait for Note: off"})"#).len(), 1);
        assert!(violations(r#"Text({"Retry".to_string()})"#).is_empty());
        assert!(violations(r#"Text({some_expr})"#).is_empty());
    }

    #[test]
    fn flags_natural_language_label_in_known_helper_same_line() {
        assert_eq!(
            violations(r#"button::small("Adaptive Difficulty", on_toggle)"#).len(),
            1
        );
    }

    #[test]
    fn flags_natural_language_label_in_known_helper_multiline() {
        let source = concat!(
            "combobox::spawn_combobox(\n",
            "    commands,\n",
            "    parent,\n",
            "    parent,\n",
            "    \"Pitch detect\",\n",
            "    &algo_labels(),\n",
            "    settings.pitch_algorithm.label(),\n",
            "    on_algo_selected,\n",
            ");\n",
        );
        assert_eq!(violations(source).len(), 1);
    }

    #[test]
    fn allows_known_helper_with_localized_label() {
        let source = concat!(
            "combobox::spawn_combobox(\n",
            "    commands,\n",
            "    parent,\n",
            "    parent,\n",
            "    &loc.msg(\"options-pitch-detect\"),\n",
            "    &algo_labels(),\n",
            "    settings.pitch_algorithm.label(),\n",
            "    on_algo_selected,\n",
            ");\n",
        );
        assert!(violations(source).is_empty());
    }

    #[test]
    fn allows_known_helper_with_single_word_label() {
        assert!(violations(r#"button::default("Retry", on_retry)"#).is_empty());
    }

    #[test]
    fn ignores_comment_lines() {
        assert!(violations(r#"// Text::new("some words here")"#).is_empty());
    }

    // ── Message registration enforcement ─────────────────────────────────────

    use super::message_registration_violations;

    fn sources(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(p, s)| (p.to_string(), s.to_string()))
            .collect()
    }

    #[test]
    fn flags_a_declared_message_with_no_registration_anywhere() {
        let src = sources(&[("a.rs", "#[derive(Message)]\npub struct Foo;\n")]);
        assert_eq!(message_registration_violations(&src).len(), 1);
    }

    #[test]
    fn allows_a_declared_message_registered_in_the_same_file() {
        let src = sources(&[(
            "a.rs",
            "#[derive(Message)]\npub struct Foo;\napp.add_message::<Foo>();\n",
        )]);
        assert!(message_registration_violations(&src).is_empty());
    }

    #[test]
    fn allows_registration_in_a_different_file_via_a_qualified_path() {
        let src = sources(&[
            ("a/foo.rs", "#[derive(Message)]\npub struct Foo;\n"),
            ("b/plugin.rs", "app.add_message::<foo::Foo>();\n"),
        ]);
        assert!(message_registration_violations(&src).is_empty());
    }

    #[test]
    fn allows_a_multi_derive_message_with_extra_derives() {
        let src = sources(&[(
            "a.rs",
            "#[derive(Message, Debug, Clone, Copy)]\npub struct Foo;\napp.add_message::<Foo>();\n",
        )]);
        assert!(message_registration_violations(&src).is_empty());
    }

    #[test]
    fn ignores_a_message_declared_only_inside_mod_tests() {
        let src = sources(&[(
            "a.rs",
            "mod tests {\n    #[derive(Message)]\n    struct Foo;\n}\n",
        )]);
        assert!(message_registration_violations(&src).is_empty());
    }

    #[test]
    fn does_not_confuse_a_non_message_derive_for_message() {
        let src = sources(&[("a.rs", "#[derive(Resource, Default)]\npub struct Foo;\n")]);
        assert!(message_registration_violations(&src).is_empty());
    }
}

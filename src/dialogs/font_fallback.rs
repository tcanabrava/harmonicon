// SPDX-License-Identifier: MIT

//! Glyph fallback for the small set of symbols the bundled `FreeSans.otf`
//! doesn't cover (emoji-range icons and a couple of dingbats used as button
//! icons). We can't rely on the OS/fontconfig to fill these in — inside the
//! Flatpak sandbox there's no guarantee a matching system font is even
//! visible — so instead a few tiny subsetted fonts are bundled and an
//! automatic system splits any offending [`Text`] into runs, rendering the
//! known-missing characters from the matching fallback font via [`TextSpan`]
//! children.

use bevy::prelude::*;

/// Which bundled fallback font a character needs, if any.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum FallbackKind {
    Emoji,
    Symbols,
    Arrows,
}

/// Characters missing from `FreeSans.otf` that `fallback_emoji.ttf` covers.
/// Keep this in sync with the codepoints actually subsetted into that font
/// (see the font-fallback notes in CLAUDE.md, or regenerate with `pyftsubset`
/// using this exact list) — a character used in a label but missing from both
/// this list and the font file will render as tofu again.
const EMOJI_CHARS: &[char] = &[
    '\u{1F3B2}', // 🎲 dice (Drill)
    '\u{1F4C1}', // 📁 closed folder (Browse)
    '\u{1F50A}', // 🔊 speaker (Listen)
    '\u{1F3B5}', // 🎵 musical note (Perform)
    '\u{1F512}', // 🔒 lock (Lock)
    '\u{1F4BE}', // 💾 floppy disk (Save)
    '\u{1F4C2}', // 📂 open folder (Load)
    '\u{1F3A4}', // 🎤 microphone (Practice)
];
/// Characters missing from `FreeSans.otf` that `fallback_symbols.ttf` covers.
const SYMBOL_CHARS: &[char] = &[
    '\u{2713}', // ✓ check mark (tuner "in tune")
    '\u{270E}', // ✎ pencil (Edit)
    '\u{23F8}', // ⏸ pause bars (Pause)
    '\u{2717}', // ✗ cross mark (Practice "missed")
];
/// Characters missing from `FreeSans.otf` that `fallback_arrows.ttf` covers.
const ARROW_CHARS: &[char] = &[
    '\u{21BB}', // ↻ clockwise open circle arrow (technique cycle)
];

fn fallback_kind(c: char) -> Option<FallbackKind> {
    if EMOJI_CHARS.contains(&c) {
        Some(FallbackKind::Emoji)
    } else if SYMBOL_CHARS.contains(&c) {
        Some(FallbackKind::Symbols)
    } else if ARROW_CHARS.contains(&c) {
        Some(FallbackKind::Arrows)
    } else {
        None
    }
}

/// Splits `text` into runs of consecutive characters that need the same font
/// (or no fallback at all). Pure and independent of the actual font handles
/// so it's cheap to unit test.
fn split_runs(text: &str) -> Vec<(String, Option<FallbackKind>)> {
    let mut runs: Vec<(String, Option<FallbackKind>)> = Vec::new();
    for c in text.chars() {
        let kind = fallback_kind(c);
        match runs.last_mut() {
            Some((s, k)) if *k == kind => s.push(c),
            _ => runs.push((c.to_string(), kind)),
        }
    }
    runs
}

/// Handles to the bundled fallback fonts, loaded at startup.
#[derive(Resource)]
struct FallbackFonts {
    emoji: Handle<Font>,
    symbols: Handle<Font>,
    arrows: Handle<Font>,
}

impl FallbackFonts {
    fn handle(&self, kind: FallbackKind) -> Handle<Font> {
        match kind {
            FallbackKind::Emoji => self.emoji.clone(),
            FallbackKind::Symbols => self.symbols.clone(),
            FallbackKind::Arrows => self.arrows.clone(),
        }
    }
}

fn load_fallback_fonts(mut fonts: ResMut<Assets<Font>>, mut commands: Commands) {
    const EMOJI_BYTES: &[u8] = include_bytes!("../../assets/fonts/fallback_emoji.ttf");
    const SYMBOLS_BYTES: &[u8] = include_bytes!("../../assets/fonts/fallback_symbols.ttf");
    const ARROWS_BYTES: &[u8] = include_bytes!("../../assets/fonts/fallback_arrows.ttf");
    commands.insert_resource(FallbackFonts {
        emoji: fonts.add(Font::from_bytes(EMOJI_BYTES.to_vec())),
        symbols: fonts.add(Font::from_bytes(SYMBOLS_BYTES.to_vec())),
        arrows: fonts.add(Font::from_bytes(ARROWS_BYTES.to_vec())),
    });
}

/// Marks a `TextSpan` this system spawned, so it's never mistaken for
/// user-authored text and never itself re-split (it's always a single
/// fallback-only or fallback-free run already).
#[derive(Component)]
struct GeneratedSpan;

/// Rewrites any changed [`Text`] containing a known-missing character: the
/// entity's own `Text`/`TextFont` become the first run, and each further run
/// is appended as a [`TextSpan`] child sourcing the matching fallback font.
/// Re-runs whenever `Text` changes (e.g. a live readout), so it stays correct
/// for reactive labels, not just ones set once at spawn.
fn apply_font_fallback(
    mut commands: Commands,
    fallback: Option<Res<FallbackFonts>>,
    mut texts: Query<(Entity, &Text, &mut TextFont, &TextColor, Option<&Children>), Changed<Text>>,
    generated: Query<(), With<GeneratedSpan>>,
) {
    let Some(fallback) = fallback else { return };
    for (entity, text, mut font, color, children) in &mut texts {
        let runs = split_runs(&text.0);
        if runs.len() <= 1 && runs.first().is_none_or(|(_, k)| k.is_none()) {
            continue;
        }

        if let Some(children) = children {
            for &child in children {
                if generated.contains(child) {
                    commands.entity(child).despawn();
                }
            }
        }

        let (first_text, first_kind) = &runs[0];
        if first_text != &text.0 {
            commands
                .entity(entity)
                .insert(Text::new(first_text.clone()));
        }
        font.font = match first_kind {
            Some(kind) => FontSource::Handle(fallback.handle(*kind)),
            None => FontSource::default(),
        };

        let base_size = font.font_size;
        let base_color = color.0;
        commands.entity(entity).with_children(|parent| {
            for (run_text, kind) in &runs[1..] {
                let font_source = match kind {
                    Some(kind) => FontSource::Handle(fallback.handle(*kind)),
                    None => FontSource::default(),
                };
                parent.spawn((
                    TextSpan::new(run_text.clone()),
                    TextFont {
                        font: font_source,
                        font_size: base_size,
                        ..default()
                    },
                    TextColor(base_color),
                    GeneratedSpan,
                ));
            }
        });
    }
}

pub struct FontFallbackPlugin;

impl Plugin for FontFallbackPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_fallback_fonts)
            .add_systems(Update, apply_font_fallback);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_runs_returns_one_run_for_plain_text() {
        let runs = split_runs("Listen");
        assert_eq!(runs, vec![("Listen".to_string(), None)]);
    }

    #[test]
    fn split_runs_separates_a_leading_emoji_from_the_rest() {
        let runs = split_runs("\u{1F50A} Listen");
        assert_eq!(
            runs,
            vec![
                ("\u{1F50A}".to_string(), Some(FallbackKind::Emoji)),
                (" Listen".to_string(), None),
            ]
        );
    }

    #[test]
    fn split_runs_merges_consecutive_same_kind_characters() {
        // Two emoji in a row should stay one run, not split per character.
        let runs = split_runs("\u{1F3B2}\u{1F50A} Drill");
        assert_eq!(
            runs,
            vec![
                ("\u{1F3B2}\u{1F50A}".to_string(), Some(FallbackKind::Emoji)),
                (" Drill".to_string(), None),
            ]
        );
    }

    #[test]
    fn split_runs_handles_symbols_and_emoji_in_one_string() {
        let runs = split_runs("\u{2713} \u{1F50A}");
        assert_eq!(
            runs,
            vec![
                ("\u{2713}".to_string(), Some(FallbackKind::Symbols)),
                (" ".to_string(), None),
                ("\u{1F50A}".to_string(), Some(FallbackKind::Emoji)),
            ]
        );
    }

    #[test]
    fn split_runs_of_empty_string_is_empty() {
        assert!(split_runs("").is_empty());
    }

    #[test]
    fn song_editor_toolbar_labels_all_split_their_leading_icon() {
        // Regression: Save/Load/Edit/Perform/Lock/Pause all lead with an icon
        // glyph FreeSans doesn't have — each must produce exactly a 2-run
        // split (icon, then plain text), not fall through as one unsplit run.
        for label in [
            "\u{1F4BE} Save",
            "\u{1F4C2} Load",
            "\u{270E} Edit",
            "\u{1F3B5} Perform",
            "\u{1F512} Lock",
            "\u{23F8} Pause",
        ] {
            let runs = split_runs(label);
            assert_eq!(
                runs.len(),
                2,
                "expected an icon run + text run for {label:?}"
            );
            assert!(
                runs[0].1.is_some(),
                "icon run should need a fallback font for {label:?}"
            );
            assert!(
                runs[1].1.is_none(),
                "trailing text run should use the default font for {label:?}"
            );
        }
    }

    #[test]
    fn fallback_kind_identifies_exactly_the_known_gaps() {
        for &c in EMOJI_CHARS {
            assert_eq!(fallback_kind(c), Some(FallbackKind::Emoji));
        }
        for &c in SYMBOL_CHARS {
            assert_eq!(fallback_kind(c), Some(FallbackKind::Symbols));
        }
        for &c in ARROW_CHARS {
            assert_eq!(fallback_kind(c), Some(FallbackKind::Arrows));
        }
        // A character FreeSans already covers should need no fallback.
        assert_eq!(fallback_kind('A'), None);
        assert_eq!(fallback_kind('\u{2190}'), None); // ← already in FreeSans
    }

    // ── apply_font_fallback (end-to-end through a minimal App) ──────────────────

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins(AssetPlugin::default())
            .init_asset::<Font>()
            .add_systems(Startup, load_fallback_fonts)
            .add_systems(Update, apply_font_fallback);
        app.update(); // run Startup so FallbackFonts exists
        app
    }

    #[test]
    fn plain_text_is_left_alone() {
        let mut app = test_app();
        let id = app
            .world_mut()
            .spawn((
                Text::new("Listen"),
                TextFont::default(),
                TextColor::default(),
            ))
            .id();
        app.update();

        let world = app.world();
        assert_eq!(world.get::<Text>(id).unwrap().0, "Listen");
        assert!(world.get::<Children>(id).is_none());
    }

    #[test]
    fn button_label_with_a_leading_icon_gets_split_into_a_span() {
        let mut app = test_app();
        let id = app
            .world_mut()
            .spawn((
                Text::new("\u{1F50A} Listen"),
                TextFont::default(),
                TextColor::default(),
            ))
            .id();
        app.update();

        let world = app.world();
        // The entity's own Text keeps just the icon; "Listen" moves to a span.
        assert_eq!(world.get::<Text>(id).unwrap().0, "\u{1F50A}");
        let font = world.get::<TextFont>(id).unwrap();
        assert_ne!(
            font.font,
            FontSource::default(),
            "icon run should use the fallback font"
        );

        let children = world.get::<Children>(id).expect("span child spawned");
        assert_eq!(children.len(), 1);
        let span = world.get::<TextSpan>(children[0]).unwrap();
        assert_eq!(span.0, " Listen");
        let span_font = world.get::<TextFont>(children[0]).unwrap();
        assert_eq!(
            span_font.font,
            FontSource::default(),
            "the text run keeps the default font"
        );
    }

    #[test]
    fn resplitting_on_change_replaces_the_previous_spans() {
        let mut app = test_app();
        let id = app
            .world_mut()
            .spawn((
                Text::new("\u{2713} A"),
                TextFont::default(),
                TextColor::default(),
            ))
            .id();
        app.update();
        let first_span = app.world().get::<Children>(id).unwrap()[0];

        // A live readout overwrites the whole string on the next frame.
        *app.world_mut().get_mut::<Text>(id).unwrap() = Text::new("\u{2713} B");
        app.update();

        let world = app.world();
        let children = world.get::<Children>(id).unwrap();
        assert_eq!(
            children.len(),
            1,
            "stale span shouldn't linger alongside the new one"
        );
        assert_eq!(world.get::<TextSpan>(children[0]).unwrap().0, " B");
        assert!(
            world.get_entity(first_span).is_err(),
            "the old span should be despawned"
        );
    }
}

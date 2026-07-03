// SPDX-License-Identifier: MIT

//! Glyph fallback for the small set of symbols the bundled `FreeSans.otf`
//! doesn't cover (emoji-range icons and a couple of math dingbats used as
//! button icons). We can't rely on the OS/fontconfig to fill these in —
//! inside the Flatpak sandbox there's no guarantee a matching system font is
//! even visible — so instead two tiny subsetted fonts are bundled and an
//! automatic system splits any offending [`Text`] into runs, rendering the
//! known-missing characters from the matching fallback font via [`TextSpan`]
//! children.

use bevy::prelude::*;

/// Which bundled fallback font a character needs, if any.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum FallbackKind {
    Emoji,
    Symbols,
}

/// Characters missing from `FreeSans.otf` that `fallback_emoji.ttf` covers.
const EMOJI_CHARS: &[char] = &['\u{1F3B2}', '\u{1F4C1}', '\u{1F50A}']; // 🎲 📁 🔊
/// Characters missing from `FreeSans.otf` that `fallback_symbols.ttf` covers.
const SYMBOL_CHARS: &[char] = &['\u{2713}', '\u{21BB}']; // ✓ ↻

fn fallback_kind(c: char) -> Option<FallbackKind> {
    if EMOJI_CHARS.contains(&c) {
        Some(FallbackKind::Emoji)
    } else if SYMBOL_CHARS.contains(&c) {
        Some(FallbackKind::Symbols)
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
}

impl FallbackFonts {
    fn handle(&self, kind: FallbackKind) -> Handle<Font> {
        match kind {
            FallbackKind::Emoji => self.emoji.clone(),
            FallbackKind::Symbols => self.symbols.clone(),
        }
    }
}

fn load_fallback_fonts(mut fonts: ResMut<Assets<Font>>, mut commands: Commands) {
    const EMOJI_BYTES: &[u8] = include_bytes!("../../assets/fonts/fallback_emoji.ttf");
    const SYMBOLS_BYTES: &[u8] = include_bytes!("../../assets/fonts/fallback_symbols.ttf");
    commands.insert_resource(FallbackFonts {
        emoji: fonts.add(Font::from_bytes(EMOJI_BYTES.to_vec())),
        symbols: fonts.add(Font::from_bytes(SYMBOLS_BYTES.to_vec())),
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
            commands.entity(entity).insert(Text::new(first_text.clone()));
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
                    TextFont { font: font_source, font_size: base_size, ..default() },
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
    fn fallback_kind_identifies_exactly_the_known_gaps() {
        for &c in EMOJI_CHARS {
            assert_eq!(fallback_kind(c), Some(FallbackKind::Emoji));
        }
        for &c in SYMBOL_CHARS {
            assert_eq!(fallback_kind(c), Some(FallbackKind::Symbols));
        }
        // A character FreeSans already covers should need no fallback.
        assert_eq!(fallback_kind('A'), None);
        assert_eq!(fallback_kind('\u{2190}'), None); // ← already in FreeSans
    }
}

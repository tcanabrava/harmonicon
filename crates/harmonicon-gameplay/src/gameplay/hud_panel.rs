// SPDX-License-Identifier: MIT

//! Scored play's side panel: song title, the live phrase
//! banner and tab ribbon, the metronome, and the technique legend.
//!
//! Everything here is *live* material — it changes as the song plays — which
//! is what distinguishes it from `song_info::spawn_song_details`, the static
//! block that moved to the countdown and the pause menu.

use bevy::prelude::*;

use harmonicon_core::chart::Modifier;

use super::metronome_overlay::spawn_metronome;
use super::modifier_legend::spawn_modifier_legend;
use super::note_ribbon_2d::NoteRibbon2dMaterial;
use super::phrase_overlay::{spawn_phrase_banner, spawn_tab_ribbon};
use super::practice_badges::spawn_practice_badges;
use super::song_info::{SongInfo, spawn_song_header};
use harmonicon_platform::localization::Localization;

/// Which contextual pieces of scored play a run shows, decided once from
/// the run's facts by [`contextual_panels`] so the rules can be tested
/// without spawning anything.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ContextualPanels {
    /// The side panel: title, phrase banner, tab ribbon, metronome.
    pub side_panel: bool,
    /// The notation staff under the progress bar.
    pub notation_staff: bool,
    /// Note markers in the progress bar's lanes.
    pub progress_notes: bool,
    /// The technique legend in the side panel.
    pub technique_legend: bool,
    /// The karaoke strip under the staff (`karaoke`).
    pub lyrics: bool,
}

/// The contextual panels for a run.
///
/// - **Compact layout** drops everything supplementary — the side panel and
///   the staff — and keeps the lane, which is the game.
/// - **An aural lesson** (ear training) hides everything that would give
///   the answer away: the staff, and the note markers in the progress bar.
/// - **The technique legend** appears only when the chart uses a technique;
///   an empty legend is noise.
/// - **Lyrics** show whenever the chart has them: two lines are worth their
///   room even on a compact screen, and words give no note away to an aural
///   lesson.
pub(super) fn contextual_panels(
    compact: bool,
    aural: bool,
    uses_techniques: bool,
    has_lyrics: bool,
) -> ContextualPanels {
    let side_panel = !compact;
    ContextualPanels {
        side_panel,
        notation_staff: !compact && !aural,
        progress_notes: !aural,
        technique_legend: side_panel && uses_techniques,
        lyrics: has_lyrics,
    }
}

/// What the panel needs to build itself. A struct rather than positional
/// arguments, because a silently-swapped `bpm`/`beats_per_bar` pair would be
/// invisible.
pub(super) struct HudPanel<'a> {
    pub song_info: &'a SongInfo,
    pub loc: &'a Localization,
    pub beats_per_bar: usize,
    pub bpm: f32,
    /// Empty when the chart uses no techniques — the legend is then omitted
    /// rather than shown empty.
    pub legend_materials: &'a [(Handle<NoteRibbon2dMaterial>, &'static str)],
}

pub(super) fn spawn_hud_panel(parent: &mut ChildSpawnerCommands, panel: HudPanel<'_>) {
    // Title only while notes are falling; key, harp, description and author
    // are read material and live where there is time to read them.
    parent.spawn(Node::default()).with_children(|col| {
        spawn_song_header(col, panel.song_info);
    });

    // Which practice aids are on — slowed, waiting, looping — so a resumed
    // song explains its own silence or stops (`practice_badges`).
    spawn_practice_badges(parent);

    // Both driven per-frame by `phrase_overlay`.
    spawn_phrase_banner(parent);
    spawn_tab_ribbon(parent);

    parent
        .spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(6.0), ..default() })
        .with_children(|metro| {
            spawn_metronome(metro, panel.loc, panel.beats_per_bar, panel.bpm);
        });

    if !panel.legend_materials.is_empty() {
        spawn_modifier_legend(parent, panel.loc, panel.legend_materials);
    }
}

/// Every technique a chart actually uses, in chart order, for
/// [`modifier_legend::build_legend_materials`].
///
/// [`modifier_legend::build_legend_materials`]: super::modifier_legend::build_legend_materials
pub(super) fn used_modifiers(chart: &harmonicon_core::chart::HarpChart) -> Vec<Modifier> {
    chart
        .track
        .iter()
        .flat_map(|item| &item.events)
        .flat_map(|event| event.modifiers.as_deref().unwrap_or_default())
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_ordinary_run_shows_every_panel_its_chart_calls_for() {
        let panels = contextual_panels(false, false, true, false);
        assert!(panels.side_panel && panels.notation_staff && panels.progress_notes);
        assert!(panels.technique_legend);
    }

    #[test]
    fn compact_layout_keeps_the_lane_and_drops_the_supplements() {
        let panels = contextual_panels(true, false, true, false);
        assert!(!panels.side_panel);
        assert!(!panels.notation_staff);
        assert!(!panels.technique_legend, "the legend lives in the panel");
        assert!(panels.progress_notes, "the progress bar is not a supplement");
    }

    #[test]
    fn an_aural_lesson_hides_everything_that_gives_the_answer_away() {
        let panels = contextual_panels(false, true, true, false);
        assert!(!panels.notation_staff);
        assert!(!panels.progress_notes);
        assert!(panels.side_panel, "the panel names no notes");
    }

    #[test]
    fn a_chart_without_techniques_gets_no_legend() {
        assert!(!contextual_panels(false, false, false, false).technique_legend);
    }

    #[test]
    fn lyrics_show_whenever_the_chart_has_them() {
        for compact in [false, true] {
            for aural in [false, true] {
                assert!(contextual_panels(compact, aural, false, true).lyrics);
                assert!(!contextual_panels(compact, aural, false, false).lyrics);
            }
        }
    }
}

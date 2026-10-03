// SPDX-License-Identifier: MIT

//! The side panel both scored modes carry: song title, the live phrase
//! banner and tab ribbon, the metronome, and the technique legend.
//!
//! Everything here is *live* material — it changes as the song plays — which
//! is what distinguishes it from `song_info::spawn_song_details`, the static
//! block that moved to the countdown and the pause menu.
//!
//! **One spawner, one order, one side.** 2D and 3D built this column
//! separately and identically except for where it sat (right vs. top-left)
//! and where the blow/draw key went, which is how they drifted into looking
//! like different games. A change to the panel is now a change to both; what
//! stays mode-specific is the lane surface and the note visuals, per
//! `docs/gameplay_improvement_plan.md`'s implementation boundaries.

use bevy::prelude::*;

use harmonicon_core::chart::Modifier;

use super::highway_2d::spawn_blow_draw_legend;
use super::metronome_overlay::spawn_metronome;
use super::modifier_legend::spawn_modifier_legend;
use super::note_ribbon_2d::NoteRibbon2dMaterial;
use super::phrase_overlay::{spawn_phrase_banner, spawn_tab_ribbon};
use super::practice_badges::spawn_practice_badges;
use super::song_info::{SongInfo, spawn_song_header};
use harmonicon_platform::localization::Localization;

/// Which lane surface a scored run draws — the one thing that differs in
/// what the two modes show around it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LaneSurface {
    Highway2d,
    Lane3d,
}

/// Which contextual pieces of scored play a run shows, decided once from
/// the run's facts by [`contextual_panels`] so 2D and 3D cannot disagree
/// and the rules can be tested without spawning anything.
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
    /// The blow/draw key inside the side panel rather than under the 2D
    /// hole strip.
    pub blow_draw_in_panel: bool,
    /// The karaoke strip under the staff (`karaoke`).
    pub lyrics: bool,
}

/// The contextual panels for a run on `surface`.
///
/// - **Compact layout** drops everything supplementary — the side panel and
///   the staff — and keeps the lane, which is the game.
/// - **An aural lesson** (ear training) hides everything that would give
///   the answer away: the staff, and the note markers in the progress bar.
/// - **The technique legend** appears only when the chart uses a technique;
///   an empty legend is noise.
/// - **The blow/draw key** goes in the panel only in 3D, which has no hole
///   strip to print it under.
/// - **Lyrics** show whenever the chart has them: two lines are worth their
///   room even on a compact screen, and words give no note away to an aural
///   lesson.
pub(super) fn contextual_panels(
    surface: LaneSurface,
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
        blow_draw_in_panel: side_panel && surface == LaneSurface::Lane3d,
        lyrics: has_lyrics,
    }
}

/// What the panel needs to build itself. A struct rather than eight
/// positional arguments, because the two call sites are in different files
/// and a silently-swapped `bpm`/`beats_per_bar` pair would be invisible.
pub(super) struct HudPanel<'a> {
    pub song_info: &'a SongInfo,
    pub loc: &'a Localization,
    pub beats_per_bar: usize,
    pub bpm: f32,
    /// Empty when the chart uses no techniques — the legend is then omitted
    /// rather than shown empty.
    pub legend_materials: &'a [(Handle<NoteRibbon2dMaterial>, &'static str)],
    /// 2D prints the blow/draw key under its hole strip, where the colours
    /// it explains actually are. 3D has no hole strip, so it asks for the
    /// key inside the panel instead. The one honest difference between the
    /// two, and it follows from the lane surface rather than from drift.
    pub blow_draw_legend: bool,
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

    if panel.blow_draw_legend {
        spawn_blow_draw_legend(parent, panel.loc, 12.0, 4.0);
    }

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
/// Shared so the legend can't list one set of techniques in 2D and another
/// in 3D — both modes flattened this out of `chart.track` themselves, and a
/// legend that disagrees with the notes is worse than no legend.
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

    const BOTH: [LaneSurface; 2] = [LaneSurface::Highway2d, LaneSurface::Lane3d];

    #[test]
    fn an_ordinary_run_shows_every_panel_its_chart_calls_for() {
        for surface in BOTH {
            let panels = contextual_panels(surface, false, false, true, false);
            assert!(panels.side_panel && panels.notation_staff && panels.progress_notes);
            assert!(panels.technique_legend);
        }
    }

    #[test]
    fn compact_layout_keeps_the_lane_and_drops_the_supplements() {
        for surface in BOTH {
            let panels = contextual_panels(surface, true, false, true, false);
            assert!(!panels.side_panel, "{surface:?}");
            assert!(!panels.notation_staff);
            assert!(!panels.technique_legend, "the legend lives in the panel");
            assert!(!panels.blow_draw_in_panel);
            assert!(panels.progress_notes, "the progress bar is not a supplement");
        }
    }

    #[test]
    fn an_aural_lesson_hides_everything_that_gives_the_answer_away() {
        for surface in BOTH {
            let panels = contextual_panels(surface, false, true, true, false);
            assert!(!panels.notation_staff, "{surface:?}");
            assert!(!panels.progress_notes);
            assert!(panels.side_panel, "the panel names no notes");
        }
    }

    #[test]
    fn a_chart_without_techniques_gets_no_legend() {
        for surface in BOTH {
            assert!(!contextual_panels(surface, false, false, false, false).technique_legend);
        }
    }

    #[test]
    fn lyrics_show_whenever_the_chart_has_them() {
        for surface in [LaneSurface::Highway2d, LaneSurface::Lane3d] {
            for compact in [false, true] {
                for aural in [false, true] {
                    assert!(contextual_panels(surface, compact, aural, false, true).lyrics);
                    assert!(!contextual_panels(surface, compact, aural, false, false).lyrics);
                }
            }
        }
    }

    #[test]
    fn the_modes_differ_only_in_where_the_blow_draw_key_goes() {
        for compact in [false, true] {
            for aural in [false, true] {
                for techniques in [false, true] {
                    let two_d = contextual_panels(
                        LaneSurface::Highway2d,
                        compact,
                        aural,
                        techniques,
                        false,
                    );
                    let three_d =
                        contextual_panels(LaneSurface::Lane3d, compact, aural, techniques, false);
                    assert!(!two_d.blow_draw_in_panel, "2D prints it under the holes");
                    assert_eq!(three_d.blow_draw_in_panel, three_d.side_panel);
                    assert_eq!(ContextualPanels { blow_draw_in_panel: false, ..three_d }, two_d);
                }
            }
        }
    }
}

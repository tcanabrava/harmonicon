// SPDX-License-Identifier: MIT

//! A song-progress bar pinned to the top of the screen, shared by the 2D and
//! 3D gameplay views. A note-lanes strip spans the harp's hole range,
//! highest hole on top (the reverse of the Song Editor's scrollbar minimap,
//! `song_editor::interaction::scrollbar_marker`, whose "note as a
//! proportional rect" it borrows). Each note is a rect in its hole's lane,
//! sized to its duration and tinted blow/draw, drawn over the song's
//! waveform (`SongManifest::waveform`), so where a note sits and how loud
//! the backing is there read at one glance. A thin red playhead marks the
//! current position.
//!
//! Per-phrase mastery is a separate overlay shown only while paused
//! ([`PhraseOverlay`], [`sync_phrase_overlay_visibility`]): a live highway
//! stays uncluttered, and mastery earns space only where it's actionable.
//! Each phrase is a full-height column with a bar growing from the bottom;
//! height, not colour, encodes the learned fraction
//! ([`phrase_bar_height_px`]), since close fractions are hard to tell apart
//! by colour.
//!
//! A song with no music has no waveform and a `music_duration_secs` of 0;
//! [`spawn_song_progress`] then times the bar by the notes' and phrases'
//! own extent.
//!
//! While playing the bar only shows; while paused ([`ProgressBarMode`]) a
//! drag sweeps out an A–B loop range and fires [`RequestLoopRange`] rather
//! than writing `LoopConfig`, keeping the gesture and the loop policy apart.

use bevy::picking::events::{PointerClick, PointerDrag, PointerDragEnd, PointerDragStart};
use bevy::picking::pointer::PointerButton;
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use bevy::ui_render::prelude::MaterialNode;

use harmonicon_app::app::AppState;

use super::adaptive_difficulty::{AdaptiveDifficulty, PhraseSection};
use super::pause_menu::SelectedPhraseIndex;
use super::song_waveform_material::{
    SongWaveformMaterial, SongWaveformMaterialPlugin, pack_amplitudes,
};
use super::{
    GameplayClock, GameplayLogic, GameplayRoot, LoopConfig, Paused, SongEnd, loop_range_valid,
};

/// Every bar keeps at least this much height (as a fraction 0..1) even during
/// silence, so the waveform reads as a continuous shape rather than gaps.
const WAVEFORM_FLOOR: f32 = 0.04;

/// Waveform fill color — a desaturated, receding, dark gray-blue,
/// deliberately *not* the same hue as a note marker's blow color drawn over
/// it, or "there's sound here" and "this is a blow note" read as the same
/// signal. Translucent (alpha 0.55) because it sits directly behind the
/// note markers — see [`spawn_song_progress`].
const WAVEFORM_COLOR: Color = Color::srgba(0.32, 0.37, 0.45, 0.55);

/// Height (px) of the note-lanes strip — tall enough that even a 12-hole
/// chromatic's lanes stay individually legible (5px each; a 10-hole
/// diatonic gets 6px). See [`note_lane_geometry`]. Also the waveform's own
/// height now, since it renders as this strip's background rather than a
/// separate row above it.
const NOTE_LANES_HEIGHT: f32 = 60.0;

/// Border width for a phrase-section column — a fully opaque outline is all
/// that marks its (mostly transparent) extent now that learned fraction is
/// a bar height, not a fill color, so adjacent columns still read as
/// distinct slots even where their bar heights are close.
const PHRASE_RECT_BORDER: f32 = 1.5;

/// Total height (px) of the bar, pinned at the very top of the screen.
/// `pub` so gameplay HUDs can reserve this much space instead of placing
/// content under it, where the bar (painted above them — see
/// [`BAR_Z_INDEX`]) would cover it. The waveform shares the note-lanes
/// strip (see [`NOTE_LANES_HEIGHT`]).
pub const BAR_HEIGHT: f32 = NOTE_LANES_HEIGHT;

/// Narrowest a note's marker is ever drawn (fraction 0..1 of the bar),
/// regardless of how short the note actually is — a proportionally-accurate
/// but sub-pixel-wide marker would be invisible. See [`note_marker_geometry`].
const MIN_NOTE_MARKER_FRAC: f32 = 0.003;

/// A freshly-spawned note-marker rect's fill, before [`sync_note_marker_colors`]
/// corrects it to the actual active theme/colorblind colors — used only so
/// a marker isn't fully invisible for the one frame between spawning and
/// that system's first run (see its own doc comment for why spawn time
/// itself can't just resolve the real color directly). Same hues the Song
/// Editor's scrollbar minimap uses by default
/// (`song_editor::interaction::SCROLLBAR_BLOW_COLOR`/`SCROLLBAR_DRAW_COLOR`),
/// for the same "note as a colored rect" language.
const NOTE_MARKER_BLOW_FALLBACK: Color = Color::srgba(0.50, 0.75, 1.00, 0.9);
const NOTE_MARKER_DRAW_FALLBACK: Color = Color::srgba(1.00, 0.62, 0.35, 0.9);

/// Alpha applied to whichever blow/draw [`harmonicon_platform::theme::NoteColors`] a note
/// marker resolves to (the theme colors themselves carry no alpha of their
/// own).
const NOTE_MARKER_ALPHA: f32 = 0.9;

/// Single fixed color for every [`PhraseMasteryBar`] — learned fraction is
/// encoded by the bar's *height* (see [`phrase_bar_height_px`]). A
/// warm gold, distinct from blow (blue-ish)/draw (orange-ish) note colors
/// and the waveform's own gray-blue, and echoing
/// [`SELECTED_PHRASE_RECT_BORDER_COLOR`]'s gold for a cohesive "meter" look.
/// Translucent rather than solid — it still visibly covers the waveform
/// behind it (the whole point of this overlay), just as a tint over it
/// rather than fully replacing it.
const PHRASE_BAR_COLOR: Color = Color::srgba(0.85, 0.75, 0.35, 0.45);

/// A phrase's mastery bar is never shorter than this (px), even at a
/// learned fraction near zero — same "never fully disappear" reasoning as
/// [`WAVEFORM_FLOOR`], just an absolute pixel floor instead of a fractional
/// one, since this bar's height is computed directly in pixels against
/// [`BAR_HEIGHT`] rather than as a percentage of a fixed-height row.
const PHRASE_BAR_MIN_HEIGHT_PX: f32 = 2.0;

/// Above the pause overlay's own backdrop (`pause_menu::setup_pause_menu`,
/// `GlobalZIndex(200)`) so the bar renders on top of — and, since bevy_ui's
/// picking backend hit-tests in the same order it paints, actually receives
/// clicks through — the pause dimming rather than being buried under it.
const BAR_Z_INDEX: i32 = 250;

/// The moving playhead; a thin vertical line, styled like the Song Editor's
/// `PlayheadLine`. Its horizontal position is driven each frame.
#[derive(Component, Default, Clone)]
pub struct ProgressPlayhead;

/// Which end of the A–B range a handle marks.
#[derive(Clone, Copy)]
enum LoopEdge {
    A,
    B,
}

/// Solid, unlike the range wash it bounds: an edge has to be findable at a
/// glance, and a translucent edge on a translucent band is neither.
const LOOP_HANDLE_COLOR: Color = Color::srgb(1.0, 0.85, 0.35);

/// Highlights the current A–B loop range within the bar — either the
/// committed `LoopConfig` range, or a live preview of an in-progress drag
/// (see [`LoopDrag`]). Absolutely positioned (unlike the waveform bars,
/// which occupy normal flex flow) so it can sit at an arbitrary offset
/// independent of them.
#[derive(Component, Default, Clone)]
pub struct LoopRangeMarker;

/// The container holding every [`PhraseSectionRect`] column — a single
/// full-bar-height overlay (covering the waveform too), shown only while
/// paused. See [`sync_phrase_overlay_visibility`].
#[derive(Component, Clone, Copy)]
struct PhraseOverlay;

/// One column in the (pause-only) per-phrase adaptive-difficulty overlay,
/// spanning its `PhraseSection`'s time range and the *whole* bar height.
/// `usize` is the section's ordinal index (into `AdaptiveDifficulty::
/// sections`/`learned`), used by [`update_phrase_mastery_bars`] to resize
/// its [`PhraseMasteryBar`] child without respawning whenever a phrase's
/// learned fraction changes (e.g. a manual pause-menu edit).
#[derive(Component, Clone, Copy)]
struct PhraseSectionRect(usize);

/// The bar that actually grows upward from a phrase column's bottom edge —
/// same section index as its parent [`PhraseSectionRect`], kept as its own
/// component (rather than reading it back off the parent) so
/// [`update_phrase_mastery_bars`] can resize it with one more disjoint query
/// instead of walking the hierarchy. See [`phrase_bar_height_px`].
#[derive(Component, Clone, Copy)]
struct PhraseMasteryBar(usize);

/// Marks a note-marker rectangle with which direction it is, so
/// [`sync_note_marker_colors`] can tint it from the active theme/colorblind
/// setting without `spawn_song_progress` (called from `gameplay_2d`/
/// `gameplay_3d`/`jam::session`'s own already near Bevy's system-param-limit
/// setup systems) needing `LoadedTheme`/`ColorblindPalette` threaded in as
/// extra params just to resolve a color.
#[derive(Component, Clone, Copy)]
struct NoteMarkerRect {
    is_blow: bool,
}

/// The single entity that accepts click-and-drag input for setting a loop
/// range — the bar's own root. Every visual child (waveform bars, note
/// markers, the loop marker, the playhead) is spawned `Pickable::IGNORE`,
/// so a click can never get swallowed by whichever tiny rectangle happens
/// to be on top; it always lands on this one surface instead.
#[derive(Component, Default, Clone)]
struct ProgressBarDragSurface;

/// The timescale the waveform bars are laid out on — real decoded audio
/// duration, set once when the bar spawns. Deliberately not [`SongEnd`]
/// (last chart note + a fixed tail): a tightly-trimmed track ends before
/// that tail elapses, a padded one keeps going after it, and positioning
/// the playhead/loop marker against the wrong one drifts them out of sync
/// with the waveform underneath.
#[derive(Resource, Default)]
pub struct AudioDuration(pub f64);

/// Whether the progress bar behaves as a static visualization (normal play)
/// or accepts click-and-drag to set an A–B loop range (while paused). Kept
/// as its own resource — rather than every system here reaching for `Paused`
/// directly — so "is the bar editable right now" is one explicit concept,
/// synced from `Paused` by [`sync_progress_bar_mode`].
#[derive(Resource, Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum ProgressBarMode {
    #[default]
    Visualization,
    Edit,
}

/// An in-progress click-and-drag on the bar, sweeping out a candidate A–B
/// loop range. `origin_time`/`current_time` are the drag's two endpoints in
/// song-seconds, in the order the pointer visited them (not sorted — the
/// drag can go either direction), live-updated so [`update_loop_marker`] can
/// preview it before the mouse is released.
#[derive(Resource, Default)]
struct LoopDrag {
    active: bool,
    origin_time: f64,
    current_time: f64,
}

/// Fired when a click-and-drag on the progress bar (in edit mode) is
/// released, proposing `start_time..end_time` as the new A–B loop range.
/// Consumed by [`apply_requested_loop_range`] — the only system that writes
/// `LoopConfig::start_time`/`end_time` here — so the drag interaction itself
/// only ever *requests* a range, never applies one directly.
#[derive(Message, Debug, Clone, Copy)]
pub struct RequestLoopRange {
    pub start_time: f64,
    pub end_time: f64,
}

/// One note's timing/direction/hole, as much as the note-lanes strip needs
/// — decoupled from any richer caller type, since callers differ: scored
/// modes (2D/3D) have `gameplay::notes::ScheduledNote`, but Jam Session
/// (nothing scored there) builds these directly from the chart's track
/// events instead.
#[derive(Clone, Copy)]
pub struct NoteMarker {
    pub time: f64,
    pub duration: f64,
    pub hole: u8,
    pub is_blow: bool,
}

/// Spawns the full-width progress bar at the top of the screen: the
/// waveform (from `waveform`, one entry per bar in 0..1, see
/// `SongManifest::waveform`) as a background layer, then the (pause-only)
/// phrase-mastery overlay above it, then the note-lanes strip (`hole_count`
/// lanes, highest hole at top) on top of both, then the loop marker and
/// playhead on top of everything. Tagged `GameplayRoot` so it's torn down
/// with the scene. `duration_secs` is the audio's real length if there is
/// one (see [`AudioDuration`]); a song with no background music falls back
/// to the furthest extent of `notes`/`sections` instead (see
/// [`effective_duration`]) so the bar still has a real timescale — only the
/// waveform background stays empty in that case.
pub fn spawn_song_progress(
    commands: &mut Commands,
    waveform: &[f32],
    duration_secs: f64,
    notes: &[NoteMarker],
    hole_count: u8,
    sections: &[PhraseSection],
    learned: &[f32],
) {
    let duration_secs = effective_duration(duration_secs, notes, sections);
    commands.insert_resource(AudioDuration(duration_secs));
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(0.0),
                left: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Px(BAR_HEIGHT),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(harmonicon_platform::theme::HUD_PANEL_BG),
            GlobalZIndex(BAR_Z_INDEX),
            GameplayRoot,
            // No `Button` marker: this is a drag surface, and the three
            // observers below are picking events, which any `Node` already
            // receives (and blocks lower nodes from, via `Pickable`'s own
            // default). The legacy `bevy_ui::widget::Button` only ever
            // added `FocusPolicy::Block` + `Interaction` here, neither of
            // which anything reads.
            RelativeCursorPosition::default(),
            ProgressBarDragSurface,
        ))
        .observe(on_drag_start)
        .observe(on_drag)
        .observe(on_drag_end)
        .with_children(|bar| {
            // The waveform as the note-lanes strip's own background — a
            // single shader-drawn node (`SongWaveformMaterial`/`assets/
            // shaders/song_waveform.wesl`) instead of one `Node` per
            // bucket. A sibling of `strip` (not nested inside it) sized and
            // positioned to exactly overlap it, spawned *first* so it's the
            // bottom layer of this whole `bar`: under the phrase-mastery
            // overlay (which needs to cover it while paused) and under
            // `strip`'s own opaque note markers (which need to stay legible
            // regardless of what's playing underneath). Nesting it inside
            // `strip` instead would put it *above* the mastery overlay,
            // since paint order follows each sibling's whole subtree, not
            // individual grandchildren interleaved across siblings. The
            // material asset itself can't be created here (`spawn_song_
            // progress` only has `Commands`, not `Assets<
            // SongWaveformMaterial>`, and every caller's own setup system
            // is already near Bevy's system-param limit) — deferred via
            // `commands.queue` instead, same "no extra system param
            // needed" trick `audio_input::start_capture` uses.
            let waveform_node = bar
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        top: Val::Px(0.0),
                        left: Val::Px(0.0),
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                    Pickable::IGNORE,
                ))
                .id();
            let waveform_amplitudes = pack_amplitudes(waveform, WAVEFORM_FLOOR);
            bar.commands().queue(move |world: &mut World| {
                let mut materials = world.resource_mut::<Assets<SongWaveformMaterial>>();
                let handle = materials.add(SongWaveformMaterial {
                    color: WAVEFORM_COLOR.into(),
                    amplitudes: waveform_amplitudes,
                });
                world.entity_mut(waveform_node).insert(MaterialNode(handle));
            });

            // The per-phrase mastery overlay — spawned after the waveform
            // (so it paints above/covers it) but before `strip` (so
            // `strip`'s own opaque note markers still end up on top of it,
            // keeping notes legible even with the overlay showing). Starts
            // hidden (`Display::None`) — matches the default unpaused state
            // a song always starts in, so no 1-frame flash;
            // `sync_phrase_overlay_visibility` shows it once actually
            // paused.
            if duration_secs > 0.0 {
                bar.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        top: Val::Px(0.0),
                        left: Val::Px(0.0),
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        display: Display::None,
                        ..default()
                    },
                    PhraseOverlay,
                    Pickable::IGNORE,
                ))
                .with_children(|overlay| {
                    for (i, section) in sections.iter().enumerate() {
                        let Some((left, width)) = phrase_rect_geometry(
                            section.start_time,
                            section.end_time,
                            duration_secs,
                        ) else {
                            continue;
                        };
                        let learned_frac = learned.get(i).copied().unwrap_or(0.0);
                        // Deliberately pickable (unlike everything else in
                        // this bar) — see `on_phrase_rect_click`. Its
                        // default `should_block_lower: true` means a drag
                        // starting over a phrase column no longer reaches
                        // `ProgressBarDragSurface`, so a loop-range drag has
                        // to start in the waveform band instead — an
                        // accepted trade for making sections clickable. Not
                        // that it matters while this overlay is visible at
                        // all: it's pause-only, and dragging a loop range is
                        // also pause-only, so the two features already can't
                        // conflict in practice.
                        overlay
                            .spawn((
                                Node {
                                    position_type: PositionType::Absolute,
                                    left: Val::Percent(left * 100.0),
                                    top: Val::Px(0.0),
                                    width: Val::Percent(width * 100.0),
                                    height: Val::Percent(100.0),
                                    border: UiRect::all(Val::Px(PHRASE_RECT_BORDER)),
                                    ..default()
                                },
                                BorderColor::all(PHRASE_RECT_BORDER_COLOR),
                                PhraseSectionRect(i),
                            ))
                            .observe(on_phrase_rect_click)
                            .with_children(|column| {
                                column.spawn((
                                    Node {
                                        position_type: PositionType::Absolute,
                                        left: Val::Px(0.0),
                                        bottom: Val::Px(0.0),
                                        width: Val::Percent(100.0),
                                        height: Val::Px(phrase_bar_height_px(learned_frac)),
                                        ..default()
                                    },
                                    BackgroundColor(PHRASE_BAR_COLOR),
                                    PhraseMasteryBar(i),
                                    Pickable::IGNORE,
                                ));
                            });
                    }
                });
            }

            bar.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(NOTE_LANES_HEIGHT),
                    position_type: PositionType::Relative,
                    ..default()
                },
                Pickable::IGNORE,
            ))
            .with_children(|strip| {
                for note in notes {
                    let Some((left, width)) =
                        note_marker_geometry(note.time, note.duration, duration_secs)
                    else {
                        continue;
                    };
                    let Some((top, height)) = note_lane_geometry(note.hole, hole_count) else {
                        continue;
                    };
                    let fallback = if note.is_blow {
                        NOTE_MARKER_BLOW_FALLBACK
                    } else {
                        NOTE_MARKER_DRAW_FALLBACK
                    };
                    strip.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Percent(left * 100.0),
                            top: Val::Percent(top * 100.0),
                            width: Val::Percent(width * 100.0),
                            height: Val::Percent(height * 100.0),
                            ..default()
                        },
                        BackgroundColor(fallback),
                        NoteMarkerRect { is_blow: note.is_blow },
                        Pickable::IGNORE,
                    ));
                }
            });

            bar.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(0.0),
                    width: Val::Percent(0.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(1.0, 0.85, 0.35, 0.30)),
                Visibility::Hidden,
                Pickable::IGNORE,
                LoopRangeMarker,
            ))
            .with_children(|range| {
                // A and B handles, children of the range so they ride along
                // with it as `update_loop_marker` moves and resizes it. A
                // translucent wash alone has no ends; these are what make
                // it read as *from here to here*.
                for (edge, label) in [(LoopEdge::A, "A"), (LoopEdge::B, "B")] {
                    let (left, right, cap_left, cap_right) = match edge {
                        LoopEdge::A => (Val::ZERO, Val::Auto, Val::Auto, Val::Px(3.0)),
                        LoopEdge::B => (Val::Auto, Val::ZERO, Val::Px(3.0), Val::Auto),
                    };
                    range.spawn_empty().apply_scene(bsn! {
                        Node {
                            position_type: {PositionType::Absolute},
                            top: {Val::ZERO},
                            left: {left},
                            right: {right},
                            width: {Val::Px(3.0)},
                            height: {Val::Percent(100.0)},
                            justify_content: {JustifyContent::FlexStart},
                        }
                        BackgroundColor({LOOP_HANDLE_COLOR})
                        ~{Pickable::IGNORE}
                        Children [
                            // The letter sits outside the handle, away from the range.
                            Node {
                                position_type: {PositionType::Absolute},
                                top: {Val::Px(-1.0)},
                                left: {cap_left},
                                right: {cap_right},
                                padding: {UiRect::axes(Val::Px(3.0), Val::ZERO)},
                            }
                            BackgroundColor({LOOP_HANDLE_COLOR})
                            ~{Pickable::IGNORE}
                            Children [
                                Text({label})
                                TextFont { font_size: {FontSize::Px(10.0)} }
                                TextColor({Color::srgb(0.08, 0.07, 0.03)})
                            ]
                        ]
                    });
                }
            });

            bar.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(0.0),
                    top: Val::Px(0.0),
                    width: Val::Px(2.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.95, 0.30, 0.30)),
                Pickable::IGNORE,
                ProgressPlayhead,
            ));
        });
}

/// Sweeps the playhead from the left edge at song start to the right edge at
/// the real end of the audio ([`AudioDuration`], not [`SongEnd`] — see its
/// doc comment). Stays put at the left during the countdown (negative clock)
/// and before a song has set a finite `SongEnd`; once the real audio content
/// is behind it, clamps at the right edge rather than racing ahead of or
/// lagging the waveform underneath it.
fn update_progress(
    clock: Res<GameplayClock>,
    song_end: Res<SongEnd>,
    duration: Res<AudioDuration>,
    mut playheads: Query<&mut Node, With<ProgressPlayhead>>,
) {
    let progress = if song_end.0.is_finite() && song_end.0 > 0.0 && duration.0 > 0.0 {
        (clock.get() / duration.0).clamp(0.0, 1.0) as f32
    } else {
        0.0
    };
    let left = Val::Percent(progress * 100.0);
    for mut node in &mut playheads {
        if node.left != left {
            node.left = left;
        }
    }
}

/// Left offset and width (fractions 0.0–1.0) for the committed loop
/// marker, or `None` if it shouldn't show — no finite song length, no known
/// audio duration, or the range isn't active. Split out from the system for
/// unit testing without rendering. `song_end_finite` mirrors
/// `update_progress`'s own "does this song have an end" gate;
/// `duration_secs` is [`AudioDuration`], so the marker lines up with the
/// waveform bars it's drawn over.
pub(super) fn loop_marker_geometry(
    active: bool,
    start_time: f64,
    end_time: f64,
    song_end_finite: bool,
    duration_secs: f64,
) -> Option<(f32, f32)> {
    if !active || !song_end_finite || duration_secs <= 0.0 {
        return None;
    }
    let left = (start_time / duration_secs).clamp(0.0, 1.0) as f32;
    let right = (end_time / duration_secs).clamp(0.0, 1.0) as f32;
    Some((left, (right - left).max(0.0)))
}

/// Left offset and width (fractions 0..1) of an in-progress drag preview.
/// Always shown once dragging has started, regardless of width (even a
/// zero-width just-begun drag) — the point is live feedback for what's
/// currently selected, not a judgment call on whether it would validate as a
/// loop yet (that's `loop_range_valid`'s job, applied once the drag ends).
/// `None` only when there's no known audio duration to lay it out against.
pub(super) fn drag_marker_geometry(
    origin_time: f64,
    current_time: f64,
    duration_secs: f64,
) -> Option<(f32, f32)> {
    if duration_secs <= 0.0 {
        return None;
    }
    let a = (origin_time / duration_secs).clamp(0.0, 1.0) as f32;
    let b = (current_time / duration_secs).clamp(0.0, 1.0) as f32;
    let left = a.min(b);
    Some((left, (a.max(b) - left).max(0.0)))
}

/// Top offset and height (fractions 0..1 of the note-lanes strip) for
/// `hole`'s lane, out of `hole_count` total — the highest hole (`hole_count`
/// itself) at the top, the lowest (hole 1) at the bottom, each getting an
/// equal slice. `None` only when there are no holes to divide the strip
/// into (a chart with a zero hole count, which shouldn't happen for a real
/// harmonica but isn't this function's job to rule out). `hole` is clamped
/// into range rather than rejected, so an out-of-range value degrades to
/// the nearest real lane instead of silently dropping the note's marker.
fn note_lane_geometry(hole: u8, hole_count: u8) -> Option<(f32, f32)> {
    if hole_count == 0 {
        return None;
    }
    let lanes = hole_count as f32;
    let lane_from_top = (hole_count - hole.clamp(1, hole_count)) as f32;
    Some((lane_from_top / lanes, 1.0 / lanes))
}

/// The timescale to lay the whole bar out on: `duration_secs` verbatim if
/// there's real audio, else the furthest extent of `notes`/`sections` (a
/// song with no background music has nothing to decode, so `duration_secs`
/// reads `0.0`) — `0.0` only for a truly empty chart.
fn effective_duration(duration_secs: f64, notes: &[NoteMarker], sections: &[PhraseSection]) -> f64 {
    if duration_secs > 0.0 {
        return duration_secs;
    }
    notes
        .iter()
        .map(|n| n.time + n.duration)
        .chain(sections.iter().map(|s| s.end_time))
        .fold(0.0_f64, f64::max)
}

/// Left offset and width (fractions 0..1) for one note's marker — mirrors
/// [`phrase_rect_geometry`]'s shape, but width reflects the note's own
/// duration (matching the Song Editor scrollbar minimap's "note as a
/// proportional rect": `song_editor::interaction::scrollbar_marker`).
/// Floored to [`MIN_NOTE_MARKER_FRAC`] so a very short note still shows as
/// a visible speck, and clamped so a floored marker near the end can't
/// poke past the bar. `None` only when there's no known duration.
fn note_marker_geometry(time: f64, duration: f64, total_duration: f64) -> Option<(f32, f32)> {
    if total_duration <= 0.0 {
        return None;
    }
    let left = (time / total_duration).clamp(0.0, 1.0) as f32;
    let width = (duration / total_duration) as f32;
    Some((left, width.max(MIN_NOTE_MARKER_FRAC).min(1.0 - left)))
}

/// Left offset and width (fractions 0..1) for a phrase section's rectangle,
/// laid out on the same `duration_secs` timescale as the waveform/note
/// markers — same shape as [`loop_marker_geometry`], but unconditional on an
/// "active"/`SongEnd` flag since every section is always shown. `None` only
/// when there's no known audio duration to lay it out against.
fn phrase_rect_geometry(start_time: f64, end_time: f64, duration_secs: f64) -> Option<(f32, f32)> {
    if duration_secs <= 0.0 {
        return None;
    }
    let left = (start_time / duration_secs).clamp(0.0, 1.0) as f32;
    let right = (end_time / duration_secs).clamp(0.0, 1.0) as f32;
    Some((left, (right - left).max(0.0)))
}

/// Height (px) of a phrase's [`PhraseMasteryBar`] for a given learned
/// fraction (clamped 0..1): linear against [`BAR_HEIGHT`] — 100% learned
/// fills the whole column (covering the waveform behind it), a fresh
/// phrase near 0% still shows [`PHRASE_BAR_MIN_HEIGHT_PX`] as a thin line
/// rather than vanishing outright.
fn phrase_bar_height_px(learned: f32) -> f32 {
    (learned.clamp(0.0, 1.0) * BAR_HEIGHT).max(PHRASE_BAR_MIN_HEIGHT_PX)
}

/// Fully opaque border color for every phrase-section rectangle — constant
/// regardless of learned%, so the boundary between sections is always
/// crisp even when two neighbors' fills are nearly the same color.
const PHRASE_RECT_BORDER_COLOR: Color = Color::srgba(1.0, 1.0, 1.0, 1.0);

/// Border color for whichever section is currently selected (see
/// [`update_selected_phrase_border`]) — a bright gold, distinct from the
/// plain white every other section's border stays.
const SELECTED_PHRASE_RECT_BORDER_COLOR: Color = Color::srgba(1.0, 0.85, 0.25, 1.0);

/// Converts a `RelativeCursorPosition::normalized` reading (-0.5..0.5
/// within the node's bounds; not clamped when the pointer is outside them,
/// which happens constantly mid-drag) into a clamped 0..`duration_secs`
/// time. `None` for an untracked cursor or a non-positive duration.
fn cursor_to_time(normalized_x: Option<f32>, duration_secs: f64) -> Option<f64> {
    let x = normalized_x?;
    if duration_secs <= 0.0 {
        return None;
    }
    let frac = (x as f64 + 0.5).clamp(0.0, 1.0);
    Some(frac * duration_secs)
}

/// Kept in step with `Paused`: the bar is only editable while the game is
/// actually paused — dragging a loop range while notes keep flying by would
/// mean fighting the clock the whole time.
fn sync_progress_bar_mode(paused: Res<Paused>, mut mode: ResMut<ProgressBarMode>) {
    let wanted = if paused.0 { ProgressBarMode::Edit } else { ProgressBarMode::Visualization };
    if *mode != wanted {
        *mode = wanted;
    }
}

fn on_drag_start(
    ev: On<PointerDragStart>,
    mode: Res<ProgressBarMode>,
    duration: Res<AudioDuration>,
    surfaces: Query<&RelativeCursorPosition, With<ProgressBarDragSurface>>,
    mut drag: ResMut<LoopDrag>,
) {
    if *mode != ProgressBarMode::Edit || ev.button != PointerButton::Primary {
        return;
    }
    let Ok(rel) = surfaces.get(ev.entity) else {
        return;
    };
    let Some(time) = cursor_to_time(rel.normalized.map(|n| n.x), duration.0) else {
        return;
    };
    *drag = LoopDrag { active: true, origin_time: time, current_time: time };
}

fn on_drag(
    ev: On<PointerDrag>,
    mode: Res<ProgressBarMode>,
    duration: Res<AudioDuration>,
    surfaces: Query<&RelativeCursorPosition, With<ProgressBarDragSurface>>,
    mut drag: ResMut<LoopDrag>,
) {
    if *mode != ProgressBarMode::Edit || !drag.active || ev.button != PointerButton::Primary {
        return;
    }
    let Ok(rel) = surfaces.get(ev.entity) else {
        return;
    };
    if let Some(time) = cursor_to_time(rel.normalized.map(|n| n.x), duration.0) {
        drag.current_time = time;
    }
}

/// Releasing the mouse ends the drag and requests the range it swept out —
/// see [`RequestLoopRange`]'s doc comment for why this doesn't just write
/// `LoopConfig` directly.
fn on_drag_end(
    ev: On<PointerDragEnd>,
    mut drag: ResMut<LoopDrag>,
    mut requests: MessageWriter<RequestLoopRange>,
) {
    if !drag.active || ev.button != PointerButton::Primary {
        return;
    }
    drag.active = false;
    requests.write(RequestLoopRange {
        start_time: drag.origin_time.min(drag.current_time),
        end_time: drag.origin_time.max(drag.current_time),
    });
}

/// Applies a requested loop range to `LoopConfig`. `loop_range_valid` decides
/// `active`, so a degenerate (zero-width) drag cleanly ends up inactive
/// rather than a stale range with a confusing on-screen marker.
fn apply_requested_loop_range(
    mut requests: MessageReader<RequestLoopRange>,
    mut loop_cfg: ResMut<LoopConfig>,
) {
    for req in requests.read() {
        loop_cfg.start_time = req.start_time;
        loop_cfg.end_time = req.end_time;
        loop_cfg.active = loop_range_valid(req.start_time, req.end_time);
    }
}

/// Keeps the loop marker in step with either an in-progress drag (live
/// preview, takes priority) or the committed `LoopConfig` (once the drag
/// ends and `apply_requested_loop_range` has caught up — ordered `.before`
/// this system so both happen in the same frame). Not gated on `Paused` for
/// the `LoopConfig` branch — a range set before pausing again should still
/// render — but dragging itself is only ever possible while paused.
fn update_loop_marker(
    loop_cfg: Res<LoopConfig>,
    song_end: Res<SongEnd>,
    duration: Res<AudioDuration>,
    drag: Res<LoopDrag>,
    mut markers: Query<(&mut Node, &mut Visibility), With<LoopRangeMarker>>,
) {
    if !loop_cfg.is_changed() && !song_end.is_changed() && !drag.is_changed() {
        return;
    }
    let geometry = if drag.active {
        drag_marker_geometry(drag.origin_time, drag.current_time, duration.0)
    } else {
        loop_marker_geometry(
            loop_cfg.active,
            loop_cfg.start_time,
            loop_cfg.end_time,
            song_end.0.is_finite() && song_end.0 > 0.0,
            duration.0,
        )
    };
    for (mut node, mut vis) in &mut markers {
        match geometry {
            Some((left, width)) => {
                node.left = Val::Percent(left * 100.0);
                node.width = Val::Percent(width * 100.0);
                *vis = Visibility::Visible;
            }
            None => *vis = Visibility::Hidden,
        }
    }
}

/// Resizes each [`PhraseMasteryBar`] when `AdaptiveDifficulty` changes (a
/// manual pause-menu edit, or a fresh song's initial load) without
/// respawning — the bars themselves are only ever (re)created in
/// [`spawn_song_progress`], since their count/geometry only changes when
/// the song itself does. Colour is fixed ([`PHRASE_BAR_COLOR`]), so this
/// never needs `ColorblindPalette`.
fn update_phrase_mastery_bars(
    adaptive: Res<AdaptiveDifficulty>,
    mut bars: Query<(&PhraseMasteryBar, &mut Node)>,
) {
    if !adaptive.is_changed() {
        return;
    }
    for (bar, mut node) in &mut bars {
        let learned = adaptive.learned.get(bar.0).copied().unwrap_or(0.0);
        node.height = Val::Px(phrase_bar_height_px(learned));
    }
}

/// Shows the per-phrase mastery overlay only while actually paused — see
/// the module doc comment for why (Rocksmith keeps mastery/difficulty
/// info out of the live note highway entirely, only surfacing it somewhere
/// you're actually choosing what to practice). A freshly-spawned overlay
/// already starts `Display::None` (a song always starts unpaused, so
/// there's nothing to correct on the first frame) — this only needs to
/// react to `Paused` actually changing thereafter.
fn sync_phrase_overlay_visibility(
    paused: Res<Paused>,
    mut overlays: Query<&mut Node, With<PhraseOverlay>>,
) {
    if !paused.is_changed() {
        return;
    }
    for mut node in &mut overlays {
        node.display = if paused.0 { Display::Flex } else { Display::None };
    }
}

/// Keeps every note-marker rect's color matching whatever the active theme
/// (or, with `ColorblindPalette` on, the fixed colorblind pair) currently
/// uses for blow/draw — the same source `gameplay_2d::spawn_visible_notes`
/// tints the falling notes from, via the same `harmonicon_platform::theme::
/// effective_note_colors`, so the bar's markers never drift from what the
/// rest of the screen shows. The bar carries one marker per note in the
/// whole song, so this paints all of them only on a theme or palette
/// change, and otherwise just markers spawned since the last run.
fn sync_note_marker_colors(
    theme: Res<harmonicon_platform::theme::LoadedTheme>,
    colorblind: Res<harmonicon_platform::settings::ColorblindPalette>,
    mut markers: Query<(Ref<NoteMarkerRect>, &mut BackgroundColor)>,
) {
    let all = theme.is_changed() || colorblind.is_changed();
    let colors =
        harmonicon_platform::theme::effective_note_colors(theme.note_colors(), colorblind.0);
    for (marker, mut color) in &mut markers {
        if !all && !marker.is_added() {
            continue;
        }
        let base = if marker.is_blow { colors.blow } else { colors.draw };
        *color = BackgroundColor(base.with_alpha(NOTE_MARKER_ALPHA));
    }
}

/// The pause menu's phrase selector is now driven by clicking a section's
/// rectangle here, not prev/next buttons — see `pause_menu::
/// SelectedPhraseIndex`'s doc comment. Only while paused, matching every
/// other interactive behaviour this bar has (dragging a loop range).
// not-a-widget-button: phrase rects are drawn segments of the progress bar,
// sized to the phrase rather than to a hit target; the keyboard route to a
// phrase is the pause menu's phrase list.
fn on_phrase_rect_click(
    ev: On<PointerClick>,
    rects: Query<&PhraseSectionRect>,
    mode: Res<ProgressBarMode>,
    mut selected: ResMut<SelectedPhraseIndex>,
) {
    if *mode != ProgressBarMode::Edit {
        return;
    }
    let Ok(rect) = rects.get(ev.entity) else {
        return;
    };
    selected.0 = rect.0;
}

/// A brighter border on the currently-selected phrase-section rectangle —
/// the only on-bar confirmation of which section the pause menu's "Learned"
/// slider will edit. Re-applied not just when `SelectedPhraseIndex` changes
/// but whenever fresh rectangles just appeared (a new song's bar spawned):
/// `SelectedPhraseIndex` deliberately isn't reset on a new song (see its
/// own doc comment), so it wouldn't otherwise read as "changed" once
/// there's something new to paint it onto.
fn update_selected_phrase_border(
    selected: Res<SelectedPhraseIndex>,
    mut rects: Query<(&PhraseSectionRect, &mut BorderColor)>,
    fresh_rects: Query<(), Added<PhraseSectionRect>>,
) {
    if !selected.is_changed() && fresh_rects.is_empty() {
        return;
    }
    for (rect, mut border) in &mut rects {
        *border = BorderColor::all(if rect.0 == selected.0 {
            SELECTED_PHRASE_RECT_BORDER_COLOR
        } else {
            PHRASE_RECT_BORDER_COLOR
        });
    }
}

pub struct SongProgressPlugin;

impl Plugin for SongProgressPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(SongWaveformMaterialPlugin)
            .init_resource::<AudioDuration>()
            .init_resource::<ProgressBarMode>()
            .init_resource::<LoopDrag>()
            .add_message::<RequestLoopRange>()
            .add_systems(
                Update,
                update_progress
                    .after(GameplayLogic)
                    .run_if(in_state(AppState::Playing).and_then(|p: Res<Paused>| !p.0)),
            )
            .add_systems(
                Update,
                (sync_progress_bar_mode, apply_requested_loop_range, update_loop_marker)
                    .chain()
                    .run_if(in_state(AppState::Playing)),
            )
            .add_systems(
                Update,
                (
                    update_phrase_mastery_bars,
                    update_selected_phrase_border,
                    sync_note_marker_colors,
                    sync_phrase_overlay_visibility,
                )
                    .run_if(in_state(AppState::Playing)),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── note_marker_geometry ───────────────────────────────────────────────────

    #[test]
    fn note_marker_hidden_without_a_known_duration() {
        assert_eq!(note_marker_geometry(0.0, 1.0, 0.0), None);
    }

    #[test]
    fn note_marker_geometry_is_a_fraction_of_the_total_duration() {
        // A 4s note starting at 8s of a 64s song: left an eighth of the way
        // in, a sixteenth wide.
        let (left, width) = note_marker_geometry(8.0, 4.0, 64.0).unwrap();
        assert!((left - 0.125).abs() < 1e-6);
        assert!((width - 0.0625).abs() < 1e-6);
    }

    #[test]
    fn note_marker_floors_the_width_of_a_very_short_note() {
        // A near-instantaneous note in a long song would be invisibly thin
        // without the floor.
        let (_, width) = note_marker_geometry(0.0, 0.001, 10_000.0).unwrap();
        assert!(width >= MIN_NOTE_MARKER_FRAC);
    }

    #[test]
    fn note_marker_never_pokes_past_the_bars_end() {
        // A floored marker on the song's very last instant must stay
        // within the bar (left + width <= 1.0).
        let (left, width) = note_marker_geometry(9_999.999, 0.0001, 10_000.0).unwrap();
        assert!(left + width <= 1.0);
    }

    #[test]
    fn note_marker_geometry_handles_a_note_spanning_the_whole_song() {
        // A note whose own duration equals the whole song (a degenerate but
        // possible input) shouldn't panic or invert.
        let (left, width) = note_marker_geometry(0.0, 10.0, 10.0).unwrap();
        assert_eq!(left, 0.0);
        assert!((width - 1.0).abs() < 1e-6);
    }

    // ── note_lane_geometry ─────────────────────────────────────────────────────

    #[test]
    fn note_lane_hidden_with_a_zero_hole_count() {
        assert_eq!(note_lane_geometry(1, 0), None);
    }

    #[test]
    fn highest_hole_is_the_top_lane() {
        let (top, height) = note_lane_geometry(10, 10).unwrap();
        assert_eq!(top, 0.0);
        assert!((height - 0.1).abs() < 1e-6);
    }

    #[test]
    fn lowest_hole_is_the_bottom_lane() {
        // Hole 1 of 10 is the *last* lane (bottom), not the first —
        // opposite the Song Editor's own scrollbar minimap ordering.
        let (top, height) = note_lane_geometry(1, 10).unwrap();
        assert!((top - 0.9).abs() < 1e-6);
        assert!((height - 0.1).abs() < 1e-6);
    }

    #[test]
    fn middle_hole_lands_in_its_own_slice() {
        // Hole 5 of 10: 5 lanes down from the highest (holes 10..6 above
        // it), so lane_from_top = 10 - 5 = 5.
        let (top, height) = note_lane_geometry(5, 10).unwrap();
        assert!((top - 0.5).abs() < 1e-6);
        assert!((height - 0.1).abs() < 1e-6);
    }

    #[test]
    fn lanes_are_narrower_on_a_chromatic_harp() {
        let (_, height_10) = note_lane_geometry(1, 10).unwrap();
        let (_, height_12) = note_lane_geometry(1, 12).unwrap();
        assert!(height_12 < height_10);
    }

    #[test]
    fn note_lane_geometry_clamps_an_out_of_range_hole() {
        // Defensive: a hole past hole_count degrades to the top lane
        // rather than vanishing the marker entirely.
        assert_eq!(note_lane_geometry(99, 10), note_lane_geometry(10, 10));
    }

    // ── effective_duration (the no-background-music fallback) ────────────────

    fn marker(time: f64, duration: f64) -> NoteMarker {
        NoteMarker { time, duration, hole: 1, is_blow: true }
    }

    fn section(start_time: f64, end_time: f64) -> PhraseSection {
        PhraseSection { name: String::new(), start_time, end_time, note_count: 0 }
    }

    #[test]
    fn effective_duration_prefers_real_audio_duration_when_present() {
        let notes = [marker(0.0, 1.0), marker(50.0, 1.0)];
        assert_eq!(effective_duration(64.0, &notes, &[]), 64.0);
    }

    #[test]
    fn effective_duration_falls_back_to_the_furthest_note_end_without_music() {
        let notes = [marker(0.0, 1.0), marker(20.0, 4.0), marker(5.0, 1.0)];
        assert_eq!(effective_duration(0.0, &notes, &[]), 24.0);
    }

    #[test]
    fn effective_duration_falls_back_to_the_furthest_phrase_section_too() {
        // A phrase section can extend past every individual note (e.g. a
        // trailing rest) — the fallback must cover it too, not just notes.
        let notes = [marker(0.0, 1.0)];
        let sections = [section(0.0, 30.0)];
        assert_eq!(effective_duration(0.0, &notes, &sections), 30.0);
    }

    #[test]
    fn effective_duration_is_zero_for_a_truly_empty_chart() {
        assert_eq!(effective_duration(0.0, &[], &[]), 0.0);
    }

    // ── phrase_rect_geometry / phrase_bar_height_px ───────────────────────────

    #[test]
    fn phrase_rect_hidden_without_a_known_audio_duration() {
        assert_eq!(phrase_rect_geometry(0.0, 8.0, 0.0), None);
    }

    #[test]
    fn phrase_rect_geometry_is_a_fraction_of_the_audio_duration() {
        let (left, width) = phrase_rect_geometry(8.0, 16.0, 64.0).unwrap();
        assert!((left - 0.125).abs() < 1e-6);
        assert!((width - 0.125).abs() < 1e-6);
    }

    #[test]
    fn phrase_bar_height_is_zero_learned_floors_to_the_minimum() {
        assert_eq!(phrase_bar_height_px(0.0), PHRASE_BAR_MIN_HEIGHT_PX);
    }

    #[test]
    fn phrase_bar_height_is_full_bar_height_when_fully_learned() {
        assert!((phrase_bar_height_px(1.0) - BAR_HEIGHT).abs() < 1e-6);
    }

    #[test]
    fn phrase_bar_height_is_linear_in_between() {
        let half = phrase_bar_height_px(0.5);
        assert!((half - BAR_HEIGHT * 0.5).abs() < 1e-6);
    }

    #[test]
    fn phrase_bar_height_clamps_out_of_range_input() {
        assert_eq!(phrase_bar_height_px(-1.0), phrase_bar_height_px(0.0));
        assert_eq!(phrase_bar_height_px(2.0), phrase_bar_height_px(1.0));
    }

    // ── loop_marker_geometry (committed LoopConfig) ───────────────────────────

    #[test]
    fn loop_marker_hidden_when_inactive() {
        assert_eq!(loop_marker_geometry(false, 8.0, 16.0, true, 60.0), None);
    }

    #[test]
    fn loop_marker_hidden_without_a_finite_song_end() {
        assert_eq!(loop_marker_geometry(true, 8.0, 16.0, false, 60.0), None);
    }

    #[test]
    fn loop_marker_hidden_without_a_known_audio_duration() {
        assert_eq!(loop_marker_geometry(true, 8.0, 16.0, true, 0.0), None);
    }

    #[test]
    fn loop_marker_geometry_is_a_fraction_of_the_audio_duration() {
        // 8s..16s of a 64s track → starts an eighth of the way in, an
        // eighth wide — driven by the real audio duration, not `SongEnd`.
        let (left, width) = loop_marker_geometry(true, 8.0, 16.0, true, 64.0).unwrap();
        assert!((left - 0.125).abs() < 1e-6);
        assert!((width - 0.125).abs() < 1e-6);
    }

    // ── drag_marker_geometry (live drag preview) ──────────────────────────────

    #[test]
    fn drag_marker_hidden_without_a_known_audio_duration() {
        assert_eq!(drag_marker_geometry(4.0, 8.0, 0.0), None);
    }

    #[test]
    fn drag_marker_geometry_normalizes_direction() {
        // Dragging right-to-left (current before origin) should still yield
        // the same left/width as dragging left-to-right.
        let forward = drag_marker_geometry(8.0, 16.0, 64.0).unwrap();
        let backward = drag_marker_geometry(16.0, 8.0, 64.0).unwrap();
        assert_eq!(forward, backward);
        assert!((forward.0 - 0.125).abs() < 1e-6);
        assert!((forward.1 - 0.125).abs() < 1e-6);
    }

    #[test]
    fn drag_marker_geometry_shows_a_zero_width_preview_at_drag_start() {
        let (left, width) = drag_marker_geometry(10.0, 10.0, 64.0).unwrap();
        assert!((left - 10.0 / 64.0).abs() < 1e-6);
        assert_eq!(width, 0.0);
    }

    // ── cursor_to_time ─────────────────────────────────────────────────────────

    #[test]
    fn cursor_to_time_is_none_without_a_tracked_cursor() {
        assert_eq!(cursor_to_time(None, 60.0), None);
    }

    #[test]
    fn cursor_to_time_is_none_without_a_positive_duration() {
        assert_eq!(cursor_to_time(Some(0.0), 0.0), None);
    }

    #[test]
    fn cursor_to_time_maps_the_node_span_to_the_full_duration() {
        // -0.5 (left edge) .. 0.5 (right edge) maps to 0..duration.
        assert_eq!(cursor_to_time(Some(-0.5), 60.0), Some(0.0));
        assert_eq!(cursor_to_time(Some(0.5), 60.0), Some(60.0));
        assert_eq!(cursor_to_time(Some(0.0), 60.0), Some(30.0));
    }

    #[test]
    fn cursor_to_time_clamps_outside_the_node_bounds() {
        // Dragging past either edge of the bar clamps to the endpoints
        // instead of extrapolating past 0 or the duration.
        assert_eq!(cursor_to_time(Some(-2.0), 60.0), Some(0.0));
        assert_eq!(cursor_to_time(Some(2.0), 60.0), Some(60.0));
    }

    // ── apply_requested_loop_range ─────────────────────────────────────────────

    #[test]
    fn apply_requested_loop_range_activates_a_valid_range() {
        let mut world = World::new();
        world.insert_resource(LoopConfig::default());
        world.init_resource::<Messages<RequestLoopRange>>();
        world.write_message(RequestLoopRange { start_time: 8.0, end_time: 16.0 });
        let mut schedule = Schedule::default();
        schedule.add_systems(apply_requested_loop_range);
        schedule.run(&mut world);
        let cfg = world.resource::<LoopConfig>();
        assert!(cfg.active);
        assert_eq!(cfg.start_time, 8.0);
        assert_eq!(cfg.end_time, 16.0);
    }

    #[test]
    fn apply_requested_loop_range_leaves_a_degenerate_range_inactive() {
        let mut world = World::new();
        world.insert_resource(LoopConfig::default());
        world.init_resource::<Messages<RequestLoopRange>>();
        world.write_message(RequestLoopRange { start_time: 8.0, end_time: 8.0 });
        let mut schedule = Schedule::default();
        schedule.add_systems(apply_requested_loop_range);
        schedule.run(&mut world);
        assert!(!world.resource::<LoopConfig>().active);
    }

    // ── sync_progress_bar_mode ─────────────────────────────────────────────────

    #[test]
    fn sync_progress_bar_mode_follows_paused() {
        let mut world = World::new();
        world.insert_resource(Paused(true));
        world.insert_resource(ProgressBarMode::Visualization);
        let mut schedule = Schedule::default();
        schedule.add_systems(sync_progress_bar_mode);
        schedule.run(&mut world);
        assert_eq!(*world.resource::<ProgressBarMode>(), ProgressBarMode::Edit);

        world.insert_resource(Paused(false));
        schedule.run(&mut world);
        assert_eq!(*world.resource::<ProgressBarMode>(), ProgressBarMode::Visualization);
    }
}

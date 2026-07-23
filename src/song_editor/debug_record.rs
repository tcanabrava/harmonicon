// SPDX-License-Identifier: MIT

//! Dev-only ("--features dev") debugging aid — never wired up outside it,
//! see `mod.rs`'s conditional `mod debug_record;`. A "Debug Recording"
//! checkbox (always visible in the mod panel's top strip, regardless of
//! mode — see `mod_panel.rs`'s own comment on why) dumps the *raw*
//! microphone audio to a WAV file, written next to `recorded.harpchart`
//! (what the live detector actually produced) and `expected.harpchart`
//! (the hand-annotated ground truth, see `expected_notes`'s module docs)
//! in `assets/debug_songs/<song name>/` whenever the song is saved — so a
//! pitch-detection miss can be diagnosed against exactly what the mic
//! heard, not just what the detector reported for it.
//!
//! The checkbox itself only *arms* this: checking it never starts capturing
//! anything by itself. [`sync_raw_capture`] gates the actual capture on
//! *either* [`RecordState::active`] or [`PracticeState::active`] — Play (in
//! either Record or Play/Practice mode) is what actually starts a take.
//! Deliberately both, not just Record mode: Record mode's live note
//! capture *punches in* over whatever notes already occupy the span it's
//! recording over (see `record.rs`'s module docs), so recording there would
//! silently overwrite hand-authored "ground truth" notes with whatever the
//! live detector itself guessed — circular for benchmarking. Practice mode
//! never touches `EditorState::notes` at all (`practice::practice_tick`
//! takes no `EditorState` access), so playing along to an already-correct,
//! by-hand-authored chart with Practice mode's Play button — while this
//! checkbox is on — captures the mic audio without disturbing the ground
//! truth `note_bench` will compare it against.
//!
//! The raw audio itself is tapped from `audio_system::pipeline`'s
//! [`RawCaptureBuffer`] (a generic, also dev-only resource living there for
//! the same reason `AudioFrame` does — see its own doc comment): this
//! module only decides *when* that tap should be running and *what* to do
//! with what it collected.

use bevy::ecs::query::Has;
use bevy::picking::Pickable;
use bevy::picking::events::{Click, Pointer};
use bevy::prelude::*;
use bevy::ui::Checked;
use bevy::ui_widgets::{Checkbox, checkbox_self_update};

use crate::app::AppState;
use crate::audio_system::audio_input::{CHUNK_SIZE, HOP_SIZE};
use crate::audio_system::pipeline::RawCaptureBuffer;
use crate::audio_system::pitch_detect::WINDOW_FUNCTION;
use crate::audio_system::wav::{encode_wav, resample_linear};
use crate::audio_system::waveform::{WAVEFORM_BUCKETS, bucket_peaks};
use crate::dialogs::file_dialog::FileChosen;
use crate::dialogs::tooltip::Tooltip;
use crate::localization::LocalizationExt;
use crate::theme::SongEditorColors;
use bevy_fluent::prelude::Localization;

use super::HOLE_COL_W;
use super::SAVE_PURPOSE;
use super::harpchart::{safe_path_segment, serialize_harpchart, serialize_harpchart_notes};
use super::panel_widgets::transport_button;
use super::practice::PracticeState;
use super::record::RecordState;
use super::state::{ContentKind, EditorState};

/// Height of the debug waveform strip below the grid — not tied to
/// `WAVEFORM_H` (the header music waveform's own height), since this strip
/// lives in a different part of the chrome with its own space budget.
const DEBUG_WAVEFORM_H: f32 = 40.0;

/// Every debug recording's WAV is written out at this rate regardless of
/// what the capture device actually used — resampled via
/// `audio_system::wav::resample_linear` — so recordings from different
/// machines/devices are directly comparable rather than each carrying
/// whatever rate happened to be plugged in that day.
const DEBUG_RECORDING_SAMPLE_RATE: u32 = 48_000;

// ── Markers ───────────────────────────────────────────────────────────────────

/// The checkbox entity itself — [`Checked`]'s presence on it is the single
/// source of truth for whether debug recording is armed (no separate bool
/// resource duplicating it).
#[derive(Component)]
struct DebugRecordCheckbox;

/// The checkmark glyph inside the checkbox box, shown/hidden to match
/// [`Checked`] — the widget is headless, so nothing draws this on its own.
#[derive(Component)]
struct DebugCheckmarkGlyph;

#[derive(Component)]
struct DebugRecordStatusLabel;

/// The debug waveform strip's own row — its `Node::display` is what's
/// actually toggled by the checkbox (see [`update_debug_waveform`]); the bar
/// children underneath just go along for the ride.
#[derive(Component)]
struct DebugWaveformRow;

/// One bar of the debug waveform strip, at bucket index `.0` — a fixed
/// [`WAVEFORM_BUCKETS`]-many are spawned once and simply recolored/resized
/// in place as the buffer grows, rather than despawned/respawned like the
/// note grid's own items (there's no scrolling/windowing concern here: the
/// whole take's audio is always squashed into the same fixed bar count).
#[derive(Component)]
struct DebugWaveformBar(usize);

// ── UI ────────────────────────────────────────────────────────────────────────

/// Spawned once into the mod panel's always-visible top strip
/// (`mod_panel.rs`, alongside Save/Load) — not a mode-specific group, since
/// it needs to work from either Record or Play/Practice mode (see the
/// module docs): a real checkbox (`bevy_ui_widgets::Checkbox`, per this
/// project's own preference for that crate's widgets over hand-rolled ones)
/// plus an "Erase" button and a status label reflecting whether it's armed
/// and, while a take is actually running, how much audio has been captured
/// so far.
pub(super) fn spawn_debug_recording_controls(
    panel: &mut ChildSpawnerCommands,
    loc: &Localization,
    colors: SongEditorColors,
) {
    panel
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(6.0),
            ..default()
        })
        .with_children(|row| {
            row.spawn((
                Button,
                Checkbox,
                DebugRecordCheckbox,
                Node {
                    width: Val::Px(18.0),
                    height: Val::Px(18.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(colors.field_bg),
                BorderColor::all(Color::srgb(0.30, 0.30, 0.40)),
                Tooltip(String::from(loc.msg("editor-debug-recording-tooltip"))),
            ))
            .observe(checkbox_self_update)
            .with_children(|cb| {
                cb.spawn((
                    Text::new("\u{2713}"),
                    TextFont {
                        font_size: FontSize::Px(14.0),
                        ..default()
                    },
                    TextColor(Color::WHITE),
                    Visibility::Hidden,
                    DebugCheckmarkGlyph,
                    Pickable::IGNORE,
                ));
            });

            row.spawn((
                Text::new(String::from(loc.msg("editor-debug-recording-button"))),
                TextFont {
                    font_size: FontSize::Px(15.0),
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
        });

    transport_button(
        panel,
        loc.msg("editor-debug-recording-erase"),
        loc.msg("editor-debug-recording-erase-tooltip"),
        colors.btn_bg,
        erase_debug_recording,
    );

    panel.spawn((
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(15.0),
            ..default()
        },
        TextColor(Color::srgb(0.75, 0.78, 0.88)),
        DebugRecordStatusLabel,
    ));
}

fn erase_debug_recording(_: On<Pointer<Click>>, mut raw: ResMut<RawCaptureBuffer>) {
    raw.samples.clear();
    raw.detected_notes.clear();
}

/// Spawned once into the fixed chrome (`ui::spawn_fixed_chrome`), right
/// below the grid's own horizontal scrollbar — hidden by default
/// (`Node::display: None`), shown only while the checkbox is checked (see
/// [`update_debug_waveform`]). A plain flexbox bar chart (each bar
/// `flex_grow: 1.0`, anchored to the bottom via `AlignItems::FlexEnd`)
/// rather than the header music waveform's absolute-positioned, tempo-map-
/// aligned bars: this strip has no chart timeline to align against — it's
/// just "what did the mic capture, squashed to fit" — so it doesn't need
/// that geometry at all.
pub(super) fn spawn_debug_waveform_strip(
    root: &mut ChildSpawnerCommands,
    colors: SongEditorColors,
) {
    root.spawn((
        DebugWaveformRow,
        Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            flex_shrink: 0.0,
            display: Display::None,
            ..default()
        },
    ))
    .with_children(|row| {
        row.spawn(Node {
            width: Val::Px(HOLE_COL_W),
            flex_shrink: 0.0,
            ..default()
        });
        row.spawn((
            Node {
                flex_grow: 1.0,
                height: Val::Px(DEBUG_WAVEFORM_H),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::FlexEnd,
                column_gap: Val::Px(1.0),
                margin: UiRect::top(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.25)),
        ))
        .with_children(|bars| {
            for i in 0..WAVEFORM_BUCKETS {
                bars.spawn((
                    DebugWaveformBar(i),
                    Node {
                        flex_grow: 1.0,
                        height: Val::Px(1.0),
                        ..default()
                    },
                    BackgroundColor(colors.accent.with_alpha(0.65)),
                ));
            }
        });
    });
}

/// Shows/hides the strip with the checkbox, and — only while it's visible —
/// re-buckets the buffer into bars whenever it's actually grown since the
/// last check (`last_len`), rather than every single frame: a multi-minute
/// take's sample count is large enough that re-scanning it 60+ times a
/// second for no new audio would be wasted work.
fn update_debug_waveform(
    checkbox: Query<Has<Checked>, With<DebugRecordCheckbox>>,
    raw: Res<RawCaptureBuffer>,
    mut row: Query<&mut Node, With<DebugWaveformRow>>,
    mut bars: Query<(&DebugWaveformBar, &mut Node), Without<DebugWaveformRow>>,
    mut last_len: Local<usize>,
) {
    let Ok(checked) = checkbox.single() else {
        return;
    };
    let Ok(mut row_node) = row.single_mut() else {
        return;
    };
    row_node.display = if checked {
        Display::Flex
    } else {
        Display::None
    };
    if !checked || raw.samples.len() == *last_len {
        return;
    }
    *last_len = raw.samples.len();
    let peaks = bucket_peaks(&raw.samples, WAVEFORM_BUCKETS);
    for (bar, mut node) in &mut bars {
        let amplitude = peaks.get(bar.0).copied().unwrap_or(0.0).clamp(0.0, 1.0);
        node.height = Val::Px((amplitude * DEBUG_WAVEFORM_H).max(1.0));
    }
}

/// Re-syncs the checkmark glyph's visibility to `Checked` every frame —
/// cheap (a couple of `single()` lookups), and simpler than reacting to
/// insertion/removal of a marker component (which needs `RemovedComponents`
/// for the "unchecked" half) for a widget this small.
fn update_checkbox_glyph(
    checkbox: Query<Has<Checked>, With<DebugRecordCheckbox>>,
    mut glyph: Query<&mut Visibility, With<DebugCheckmarkGlyph>>,
) {
    let Ok(checked) = checkbox.single() else {
        return;
    };
    let Ok(mut vis) = glyph.single_mut() else {
        return;
    };
    *vis = if checked {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
}

/// Distinguishes "armed but no take running yet" from "actually capturing
/// right now" — the label the checkbox alone used to drive said "Recording"
/// the instant it was checked, which looked like clicking it had started
/// something; it hadn't (see the module docs).
fn update_debug_record_status_label(
    checkbox: Query<Has<Checked>, With<DebugRecordCheckbox>>,
    record: Res<RecordState>,
    practice: Res<PracticeState>,
    raw: Res<RawCaptureBuffer>,
    loc: Res<Localization>,
    mut labels: Query<&mut Text, With<DebugRecordStatusLabel>>,
) {
    let Ok(checked) = checkbox.single() else {
        return;
    };
    let text = if !checked {
        loc.msg("editor-debug-recording-off")
    } else if record.active || practice.active {
        let secs = raw.samples.len() as f32 / raw.sample_rate.max(1) as f32;
        loc.msg_args(
            "editor-debug-recording-status",
            &[("secs", format!("{secs:.1}"))],
        )
    } else {
        loc.msg("editor-debug-recording-armed")
    };
    for mut t in &mut labels {
        *t = Text::new(text.to_string());
    }
}

// ── Recording lifecycle ──────────────────────────────────────────────────────

/// Keeps [`RawCaptureBuffer`] in step with the checkbox and the Record
/// take's own start/stop, every frame: only actually accumulates while both
/// the checkbox is checked and a take is active, and clears out whatever a
/// previous take captured the instant a new one begins — the same
/// "recording again replaces" behaviour the ordinary note punch-in already
/// has, rather than silently appending onto stale audio from an earlier,
/// unrelated take. The checkbox itself never starts or stops anything here
/// — only Play (`RecordState::active` going true) does.
fn sync_raw_capture(
    checkbox: Query<Has<Checked>, With<DebugRecordCheckbox>>,
    record: Res<RecordState>,
    practice: Res<PracticeState>,
    mut raw: ResMut<RawCaptureBuffer>,
    mut was_active: Local<bool>,
) {
    let Ok(checked) = checkbox.single() else {
        return;
    };
    // Either transport counts — see the module docs for why both, and why
    // Practice (not Record) is the one to use when the chart's own notes
    // must stay untouched.
    let active_now = record.active || practice.active;
    let take_just_started = active_now && !*was_active;
    *was_active = active_now;
    if checked && take_just_started {
        raw.samples.clear();
        raw.detected_notes.clear();
    }
    raw.recording = checked && active_now;
}

/// Writes `recorded.harpchart` + `expected.harpchart` plus the take's raw
/// WAV into `assets/debug_songs/<song name>/` whenever the song is saved —
/// a separate `FileChosen{purpose: SAVE_PURPOSE}` consumer alongside
/// `harpchart::handle_save_chosen` (same message, same purpose, different
/// concern, same split-by-consumer pattern `harpchart`/`lesson_form`'s own
/// save handlers already use). Skipped entirely if nothing was actually
/// captured, so turning the checkbox on and off without ever taking
/// anything doesn't litter the debug folder with empty recordings.
fn write_debug_recording_on_save(
    mut chosen: MessageReader<FileChosen>,
    state: Res<EditorState>,
    raw: Res<RawCaptureBuffer>,
) {
    for ev in chosen.read() {
        if ev.purpose != SAVE_PURPOSE {
            continue;
        }
        // Both conditions below fail silently on purpose in the common case
        // (an ordinary song save with the checkbox never touched shouldn't
        // print anything) — but printed here, not just `continue`d past, so
        // turning the checkbox on without ever actually taking anything
        // doesn't look like Save quietly did nothing for no reason.
        if state.content_kind != ContentKind::Song {
            println!(
                "Debug recording: skipped — only written for ContentKind::Song saves \
                 (this save is a lesson)."
            );
            continue;
        }
        if raw.samples.is_empty() {
            println!(
                "Debug recording: skipped — nothing captured yet. Checking the box alone \
                 doesn't record anything; press Play (Record mode) or Practice (Play mode) \
                 while it's checked, then Save."
            );
            continue;
        }
        let song_name = safe_path_segment(if state.name.is_empty() {
            "untitled"
        } else {
            &state.name
        });
        let dir = std::path::Path::new("assets/debug_songs").join(&song_name);
        if let Err(e) = std::fs::create_dir_all(&dir) {
            println!("Debug recording: mkdir failed: {e}");
            continue;
        }

        // Two charts, not one: `recorded.harpchart` is whatever the live
        // detector actually produced (`state.notes`, mistakes and all —
        // unchanged from what a plain chart save would write);
        // `expected.harpchart` is the hand-annotated ground truth
        // (`state.expected_notes`, see `expected_notes`'s module docs) —
        // `note_bench` compares a detector's offline replay against the
        // latter, never the former, so a detection miss can't accidentally
        // get "confirmed" by comparing it against itself.
        let recorded_path = dir.join("recorded.harpchart");
        let recorded_json = serialize_harpchart(&state);
        if let Err(e) = std::fs::write(&recorded_path, recorded_json.as_bytes()) {
            println!("Debug recording: recorded-chart write failed: {e}");
        }
        let expected_path = dir.join("expected.harpchart");
        let expected_json = serialize_harpchart_notes(&state, &state.expected_notes);
        if let Err(e) = std::fs::write(&expected_path, expected_json.as_bytes()) {
            println!("Debug recording: expected-chart write failed: {e}");
        }

        // Resampled to a fixed rate regardless of what the capture device
        // actually used, so recordings from different machines line up.
        let resampled = resample_linear(
            &raw.samples,
            raw.sample_rate.max(1),
            DEBUG_RECORDING_SAMPLE_RATE,
        );
        let wav = encode_wav(&resampled, DEBUG_RECORDING_SAMPLE_RATE);
        let wav_path = dir.join("recording.wav");
        if let Err(e) = std::fs::write(&wav_path, &wav) {
            println!("Debug recording: wav write failed: {e}");
            continue;
        }

        // Everything else needed to reproduce the original detection
        // exactly, rather than guess at it later: which algorithm actually
        // produced the pitches this take scored against, the fixed FFT/hop/
        // window the pipeline always uses, when the take was recorded, and
        // the detected-note log itself (time since the take's first sample,
        // one entry per change — see `RawCaptureBuffer::detected_notes`).
        let timestamp_unix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let detected_notes: Vec<serde_json::Value> = raw
            .detected_notes
            .iter()
            .map(|(t, notes)| serde_json::json!({ "time": t, "notes": notes }))
            .collect();
        let metadata = serde_json::json!({
            "timestamp_unix": timestamp_unix,
            "sample_rate_hz": DEBUG_RECORDING_SAMPLE_RATE,
            "original_sample_rate_hz": raw.sample_rate,
            "algorithm": raw.algorithm.label(),
            "fft_size": CHUNK_SIZE,
            "hop_size": HOP_SIZE,
            "window": WINDOW_FUNCTION,
            "detected_notes": detected_notes,
        });
        let meta_path = dir.join("recording.json");
        if let Err(e) = std::fs::write(
            &meta_path,
            serde_json::to_string_pretty(&metadata).unwrap_or_default(),
        ) {
            println!("Debug recording: metadata write failed: {e}");
        }

        println!(
            "Debug recording written: {} + {} + {} + {}",
            recorded_path.display(),
            expected_path.display(),
            wav_path.display(),
            meta_path.display()
        );
    }
}

// ── Plugin ────────────────────────────────────────────────────────────────────

pub(super) struct DebugRecordPlugin;

impl Plugin for DebugRecordPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RawCaptureBuffer>().add_systems(
            Update,
            (
                sync_raw_capture,
                update_checkbox_glyph,
                update_debug_record_status_label,
                update_debug_waveform,
                write_debug_recording_on_save,
            )
                .run_if(in_state(AppState::SongEditor2)),
        );
    }
}

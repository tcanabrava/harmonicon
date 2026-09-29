// SPDX-License-Identifier: MIT

//! 3D notes: building `SongNotes` for a chart, spawning each note as a flat
//! ribbon plus its floating label in the `LOOKAHEAD` window, scrolling and
//! recycling them, and the per-frame tint / judged-note / ribbon animation
//! systems — the 3D twin of `gameplay_2d`'s note path.

use harmonicon_core::chart::Modifier;

use super::*;

/// Length of the bright cap at a ribbon's front edge — the attack — in world
/// units. Fixed rather than proportional, so a short note's attack is as
/// clear as a long one's.
pub(super) const RIBBON_CAP: f32 = 0.3;
/// Gap left at the back of every ribbon, in world units, so two notes back
/// to back on one hole read as two attacks rather than one long note.
pub(super) const RIBBON_END_GAP: f32 = 0.15;

/// `(ribbon_w, ribbon_len)` for a note of `duration`: the ribbon runs the
/// note's length less the end gap, never shorter than its cap plus a
/// sliver of body.
pub(super) fn note_dimensions(duration: f64) -> (f32, f32) {
    let len = (note_depth(duration) - RIBBON_END_GAP).max(RIBBON_CAP + 0.1);
    (LANE_WIDTH * NOTE_W, len)
}

/// `hole_count` comes from the loaded chart's harmonica (10 for diatonic,
/// more for chromatic) — not a fixed constant, so lanes/notes land in the
/// right place regardless of harmonica type.
pub(super) fn lane_x(hole: u8, hole_count: u8) -> f32 {
    (hole as f32 - 1.0) * LANE_WIDTH - (hole_count as f32 * LANE_WIDTH) / 2.0 + LANE_WIDTH * 0.5
}

/// A note's full length on the lane in world units: its duration at the
/// lane's speed. Clamped below so a very short note still has room for its
/// cap, and above at a lane and a half, past which the rest is off screen.
pub(super) fn note_depth(duration: f64) -> f32 {
    ((duration as f32 / LOOKAHEAD as f32) * LANE_DEPTH).clamp(0.4, LANE_DEPTH * 1.5)
}

/// World units a note travels per second.
pub(super) fn lane_speed() -> f32 {
    LANE_DEPTH / LOOKAHEAD as f32
}

/// The 3D ribbon's `technique` uniform: the shared
/// `note_ribbon::ribbon_technique`, with the rate turned into cycles per
/// world unit. The ribbon scrolls at [`lane_speed`], so crests that far
/// apart cross the hit line at exactly the charted rate.
pub(super) fn ribbon_technique_3d(modifiers: &[Modifier]) -> Vec4 {
    let mut technique = super::super::note_ribbon::ribbon_technique(modifiers);
    technique.y /= lane_speed();
    technique
}

/// Builds every note's score state (`SongNotes`) plus the render config
/// `spawn_visible_notes_3d` needs (`NoteRenderAssets3D`) — no entities yet;
/// notes spawn lazily in a `LOOKAHEAD` window around the playhead,
/// mirroring `gameplay_2d::spawn_visible_notes`.
pub(super) fn build_song_notes_3d(
    effective: &EffectiveHarmonica,
    chart: &HarpChart,
    adaptive: &AdaptiveDifficulty,
) -> (super::super::SongNotes, NoteRenderAssets3D) {
    let (notes, _) = super::super::build_scheduled_notes(effective, chart, adaptive);
    let hole_count = effective.harp_for(chart).hole_count();
    (
        super::super::SongNotes { notes, cursor: 0 },
        NoteRenderAssets3D { hole_count },
    )
}

/// Spawns 3D note visuals for any note newly within the `LOOKAHEAD` window.
/// Self-healing across a loop wrap, same as the 2D version: no persistent
/// spawn cursor, just "is this note's window open, and does it already have
/// a visual" recomputed each frame.
pub fn spawn_visible_notes_3d(
    mut already_spawned: Local<HashSet<usize>>,
    mut commands: Commands,
    clock: Res<super::super::GameplayClock>,
    song_notes: Res<super::super::SongNotes>,
    render_assets: Res<NoteRenderAssets3D>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut ribbons: ResMut<Assets<NoteRibbon3dMaterial>>,
    existing: Query<&NoteVisual3D>,
    show_numbers: Res<ShowNoteNumbers>,
    theme: Res<LoadedTheme>,
    colorblind: Res<harmonicon_platform::settings::ColorblindPalette>,
    lesson: Option<Res<harmonicon_song::lessons::LessonContext>>,
    loc: Res<Localization>,
) {
    if lesson.is_some_and(|lesson| lesson.aural) {
        return;
    }
    let colors = effective_note_colors(theme.note_colors(), colorblind.0);
    let elapsed = clock.get();
    already_spawned.clear();
    already_spawned.extend(existing.iter().map(|v| v.note_id));
    for i in super::super::notes_needing_spawn(&song_notes.notes, &already_spawned, elapsed) {
        if note_has_left_view(&song_notes.notes[i], elapsed) {
            continue;
        }
        spawn_note_visual_3d(
            &mut commands,
            &mut meshes,
            &mut ribbons,
            &render_assets,
            i,
            &song_notes.notes[i],
            show_numbers.0,
            super::super::technique_cue::note_cue(&loc, &song_notes.notes[i]),
            colors,
        );
    }
}

/// A note ribbon's colour: its blow/draw colour from `colors` (the active
/// theme's, or the fixed colorblind-safe pair — see
/// `theme::effective_note_colors`), or dim red once missed. A hit keeps the
/// base colour: the shader's hold state turns it gold while the pitch is
/// held and greys it when it drops, so the tint must not also claim it.
/// Shared by spawn and `update_note_visuals_3d` so the two can't drift.
pub(super) fn ribbon_color(missed: bool, is_blow: bool, colors: NoteColors) -> LinearRgba {
    if missed {
        return Color::srgba(0.5, 0.13, 0.13, 0.6).to_linear();
    }
    let c = if is_blow { colors.blow } else { colors.draw }.to_srgba();
    Color::srgba(c.red, c.green, c.blue, 0.95).to_linear()
}

/// Spawns one note as a flat ribbon on its lane — no separate head: the
/// ribbon's front edge (with its bright cap) is the attack and its length
/// the duration. The note root sits at the front edge, so
/// `update_notes_3d` only has to move it and the label can anchor on it.
pub(super) fn spawn_note_visual_3d(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    ribbons: &mut Assets<NoteRibbon3dMaterial>,
    assets: &NoteRenderAssets3D,
    note_id: usize,
    note: &ScheduledNote,
    show_numbers: bool,
    cue: Option<String>,
    colors: NoteColors,
) {
    let note_x = lane_x(note.hole, assets.hole_count);
    let (ribbon_w, ribbon_len) = note_dimensions(note.duration);
    let material = ribbons.add(NoteRibbon3dMaterial {
        color: ribbon_color(false, note.is_blow, colors),
        technique: ribbon_technique_3d(&note.modifiers),
        shape: Vec4::new(ribbon_len, RIBBON_CAP, 0.0, note_id as f32 * 1.7),
        hold: Vec4::ZERO,
    });
    let mesh = meshes.add(Mesh::from(Plane3d::new(
        Vec3::Y,
        Vec2::new(ribbon_w * 0.5, ribbon_len * 0.5),
    )));

    let note_entity = commands
        .spawn((
            // Clear of the hit band, hit line and hole pads (see
            // `RIBBON_Y`), so it never fights them for depth.
            Transform::from_xyz(note_x, RIBBON_Y, FAR_Z),
            NoteVisual3D { note_id },
            JudgedState::default(),
            GameplayRoot,
        ))
        .with_children(|note_e| {
            // Trailing back (−Z) from the front edge at the parent origin.
            note_e.spawn((
                Mesh3d(mesh),
                MeshMaterial3d(material),
                Transform::from_xyz(0.0, 0.0, -ribbon_len * 0.5),
                NoteRibbon3d,
            ));
        })
        .id();

    // Tab label (with the technique suffix, `-3''`) and, for a technique
    // note, the short cue under it (`→ A`, `vib 5/s`): a separate UI entity
    // (see `NoteHoleLabel3D`'s doc comment for why), positioned every frame
    // by `update_note_hole_labels_3d` — hidden until then, since it starts
    // at the origin. A technique note gets one even with note numbers off,
    // since the cue is the only place its technique is spelled out.
    if show_numbers || cue.is_some() {
        let on_right =
            super::super::technique_cue::beside_lane(note.hole, assets.hole_count) > note.hole;
        let tab = super::super::gameplay_2d::head_label(
            note.hole,
            note.is_blow,
            &note.modifiers,
            show_numbers,
        );
        commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                    ..default()
                },
                // Beside the ribbon's front edge, vertically centred on it,
                // so it never covers the technique drawn along the ribbon.
                UiTransform::from_translation(if on_right {
                    Val2::percent(0.0, -50.0)
                } else {
                    Val2::percent(-100.0, -50.0)
                }),
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
                Visibility::Hidden,
                NoteHoleLabel3D {
                    target: note_entity,
                    on_right,
                },
                GameplayRoot,
            ))
            .with_children(|l| {
                l.spawn_empty()
                    .apply_scene(bsn! {
                        Text({tab})
                        TextFont { font_size: {FontSize::Px(22.0)} }
                        TextColor({Color::WHITE})
                    })
                    .insert(NoteHoleLabelText3D);
                if let Some(cue) = cue {
                    l.spawn_empty().apply_scene(bsn! {
                        Text({cue})
                        TextFont { font_size: {FontSize::Px(18.0)} }
                        TextColor({Color::srgb(1.0, 0.92, 0.6)})
                        ~{TextLayout::no_wrap()}
                    });
                }
            });
    }
}

/// Where a label anchors relative to its note's root (the ribbon's front
/// edge, centred in the lane): just past the ribbon's side, on the side the
/// label sits.
pub(super) fn label_anchor_offset(on_right: bool) -> Vec3 {
    let reach = LANE_WIDTH * NOTE_W * 0.5 + 0.05;
    Vec3::X * if on_right { reach } else { -reach }
}

/// Converts a `Camera::world_to_viewport` result into the `Val::Px` a UI
/// `Node`'s `left`/`top` needs. `world_to_viewport` resolves through
/// `logical_viewport_rect()`, the same logical-window-pixel space `Val::Px`
/// is in — except bevy_ui additionally multiplies every `Val::Px` by
/// [`UiScale`] before converting to physical pixels, a multiplier the camera
/// projection knows nothing about. Dividing by `ui_scale` here cancels that
/// back out, so the label lands beside the note regardless of the player's
/// UI zoom level (`dialogs::ui_scale`).
pub(super) fn note_label_position(viewport_px: Vec2, ui_scale: f32) -> Vec2 {
    viewport_px / ui_scale
}

/// Positions each [`NoteHoleLabel3D`] over its target note's current screen
/// position, or hides it once the note is behind the camera, or despawns it
/// once the note itself is gone (scrolled past and recycled).
///
/// Reads the note's local `Transform`, not `GlobalTransform`:
/// `update_notes_3d` (earlier in the same `Update` chain) writes
/// `Transform.translation.z` every frame, but `GlobalTransform`
/// propagation only runs afterward in `PostUpdate` — reading it here would
/// always be one frame stale, the label trailing behind its note. Note
/// root entities have no transform parent, so the local `Transform`
/// already *is* world space; nothing to wait on.
pub fn update_note_hole_labels_3d(
    mut commands: Commands,
    camera: Query<(&Camera, &GlobalTransform), With<GameplayCamera3D>>,
    ui_scale: Res<UiScale>,
    notes: Query<&Transform, With<NoteVisual3D>>,
    mut labels: Query<(Entity, &NoteHoleLabel3D, &mut Node, &mut Visibility)>,
) {
    let Ok((camera, camera_transform)) = camera.single() else {
        return;
    };

    for (entity, label, mut node, mut visibility) in &mut labels {
        let Ok(note_transform) = notes.get(label.target) else {
            commands.entity(entity).despawn();
            continue;
        };
        let anchor = note_transform.translation + label_anchor_offset(label.on_right);
        match camera.world_to_viewport(camera_transform, anchor) {
            Ok(viewport_px) => {
                let pos = note_label_position(viewport_px, ui_scale.0);
                node.left = Val::Px(pos.x);
                node.top = Val::Px(pos.y);
                // Guarded so change detection (and the visibility-propagation
                // it triggers) doesn't fire every frame for every label while
                // nothing about their visibility actually changed.
                if *visibility != Visibility::Visible {
                    *visibility = Visibility::Visible;
                }
            }
            Err(_) => {
                if *visibility != Visibility::Hidden {
                    *visibility = Visibility::Hidden;
                }
            }
        }
    }
}

pub(super) fn note_has_left_view(note: &ScheduledNote, elapsed: f64) -> bool {
    let (_, ribbon_len) = note_dimensions(note.duration);
    let distance = (elapsed - note.time) as f32 * lane_speed();
    // Gone once the whole ribbon has run off the track's near end (just
    // past the hole pads), rather than sliding on under the camera.
    distance > ribbon_len + (PAD_Z + 0.6 - HIT_Z)
}

pub fn update_notes_3d(
    clock: Res<super::super::GameplayClock>,
    song_notes: Res<super::super::SongNotes>,
    mut commands: Commands,
    mut notes: Query<(Entity, &NoteVisual3D, &mut Transform)>,
) {
    let elapsed = clock.get();
    for (entity, visual, mut tf) in &mut notes {
        let Some(note) = song_notes.notes.get(visual.note_id) else {
            continue;
        };
        // Recycle once the whole ribbon has passed the hit zone. Score
        // state lives independently in `SongNotes`, so this despawns
        // unconditionally even while looping — `spawn_visible_notes_3d`
        // respawns it once the (rewound) clock nears it again.
        if note_has_left_view(note, elapsed) {
            commands.entity(entity).despawn();
            continue;
        }
        // The front edge lands on the hit line at the note's time.
        tf.translation.z = HIT_Z - (note.time - elapsed) as f32 * lane_speed();
    }
}

/// Keeps each ribbon's colour ([`ribbon_color`]) and live hold state in
/// step with its note. `ScheduledNote` isn't an ECS component (score state
/// lives in `SongNotes`), so this re-syncs every currently-spawned note
/// each frame rather than reacting to a change — cheap, since only a
/// `LOOKAHEAD` window's worth of notes are ever spawned.
pub fn update_note_visuals_3d(
    mut sounding: Local<HashSet<u8>>,
    song_notes: Res<super::super::SongNotes>,
    clock: Res<super::super::GameplayClock>,
    audio: Res<harmonicon_audio::AudioSettings>,
    pitch_filter: Res<super::super::HarmonicaPitchFilter>,
    active: Res<ActivePitches>,
    valid_notes: Res<ValidHarpNotes>,
    notes: Query<(&NoteVisual3D, &Children)>,
    ribbon_meshes: Query<&MeshMaterial3d<NoteRibbon3dMaterial>, With<NoteRibbon3d>>,
    mut ribbons: ResMut<Assets<NoteRibbon3dMaterial>>,
    theme: Res<LoadedTheme>,
    colorblind: Res<harmonicon_platform::settings::ColorblindPalette>,
) {
    let colors = effective_note_colors(theme.note_colors(), colorblind.0);
    let judged = judged_instant(clock.get(), &audio, Some(&pitch_filter));
    harp_pitches(&active, &valid_notes, &mut sounding);
    for (visual, children) in &notes {
        let Some(note) = song_notes.notes.get(visual.note_id) else {
            continue;
        };
        let color = ribbon_color(note.missed, note.is_blow, colors);
        let hold = hold_uniform(
            note,
            judged,
            note.expected_pitch.is_some_and(|m| sounding.contains(&m)),
            live_technique_status(&note.modifiers, &note.pitch_samples, &note.amp_samples),
        );
        // Writing through `get_mut` queues `AssetEvent::Modified` and a GPU
        // re-upload even for an unchanged value, so compare first.
        for child in children {
            if let Ok(h) = ribbon_meshes.get(*child)
                && ribbons
                    .get(&h.0)
                    .is_some_and(|m| m.color != color || m.hold != hold)
                && let Some(mut m) = ribbons.get_mut(&h.0)
            {
                m.color = color;
                m.hold = hold;
            }
        }
    }
}

/// The 3D twin of `gameplay_2d::animate_judged_notes`: widens the ribbon on
/// a hit and narrows it on a miss — its width only, since its length is the
/// note's duration and must stay true — and stamps the floating label with
/// a check or cross, once per judgment via [`JudgedState`], undone when an
/// A–B loop clears the note.
pub fn animate_judged_notes_3d(
    mut commands: Commands,
    song_notes: Res<super::super::SongNotes>,
    clock: Res<super::super::GameplayClock>,
    reduced_motion: Res<harmonicon_platform::settings::ReducedMotion>,
    mut notes: Query<(
        Entity,
        &NoteVisual3D,
        &mut JudgedState,
        Option<&Judged>,
        &Children,
    )>,
    mut ribbons: Query<&mut Transform, With<NoteRibbon3d>>,
    labels: Query<(&NoteHoleLabel3D, &Children)>,
    mut label_texts: Query<&mut Text, With<NoteHoleLabelText3D>>,
    show_numbers: Res<ShowNoteNumbers>,
) {
    let now = clock.get();
    for (entity, visual, mut state, judged, children) in &mut notes {
        let Some(note) = song_notes.notes.get(visual.note_id) else {
            continue;
        };
        let current = judged_now(note);
        let transitioned = current != state.0;
        let judged = if transitioned {
            state.0 = current;
            match current {
                Some(hit) => {
                    let j = Judged { hit, at: now };
                    commands.entity(entity).insert(j);
                    Some(j)
                }
                None => {
                    commands.entity(entity).remove::<Judged>();
                    None
                }
            }
        } else {
            judged.copied()
        };
        let scale = judged.map_or(1.0, |j| {
            judged_scale(j.hit, (now - j.at) as f32, reduced_motion.0)
        });
        for child in children {
            if let Ok(mut transform) = ribbons.get_mut(*child)
                && transform.scale.x != scale
            {
                transform.scale.x = scale;
            }
        }
        if !transitioned {
            continue;
        }
        let wanted = match current {
            Some(hit) => judged_stamp(hit).to_string(),
            None => super::super::gameplay_2d::head_label(
                note.hole,
                note.is_blow,
                &note.modifiers,
                show_numbers.0,
            ),
        };
        for (label, label_children) in &labels {
            if label.target != entity {
                continue;
            }
            for grandchild in label_children {
                if let Ok(mut text) = label_texts.get_mut(*grandchild) {
                    text.0 = wanted.clone();
                }
            }
        }
    }
}

/// Drives every ribbon's animation clock (`shape.z`) from the gameplay
/// clock, so the hold shimmer runs in time with the song and freezes on
/// pause. The technique patterns themselves are fixed along the ribbon and
/// move only because the note does.
pub fn animate_note_ribbons_3d(
    clock: Res<super::super::GameplayClock>,
    reduced_motion: Res<harmonicon_platform::settings::ReducedMotion>,
    mut materials: ResMut<Assets<NoteRibbon3dMaterial>>,
) {
    if reduced_motion.0 {
        return;
    }
    let t = clock.get() as f32;
    for (_, material) in materials.iter_mut() {
        material.shape.z = t;
    }
}

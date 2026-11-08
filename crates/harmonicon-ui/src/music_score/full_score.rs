// SPDX-License-Identifier: MIT

//! The paused full score: the whole song's notation laid out at once, which
//! unlike the live staff never follows the playhead (its parent viewport
//! scrolls it), and the beat geometry it shares with the selection band
//! drawn over it.

use super::*;

/// The same beat geometry used for the full pause score and its selection band.
pub fn score_beat_x(beat: f64, spacing: f32) -> f32 {
    PLAYHEAD_X + beat as f32 * spacing
}

#[derive(Component)]
pub(super) struct FullScoreLayer {
    grand: bool,
    root: Entity,
    dirty: bool,
    end: f64,
}

/// Static, full-song notation. Unlike the live staff it never follows the
/// playhead; its parent viewport owns scrolling.
pub fn spawn_full_score(
    parent: &mut ChildSpawnerCommands,
    bravura: &BravuraFont,
    grand: bool,
    end: f64,
) -> Entity {
    let root = spawn_score(parent, bravura, grand);
    parent.commands().queue(move |world: &mut World| {
        world.entity_mut(root).remove::<MusicScorePanel>();
        let children: Vec<_> = world.get::<Children>(root).into_iter().flatten().copied().collect();
        for child in children {
            if world.get::<MusicScoreReference>(child).is_some() {
                world.despawn(child);
                continue;
            }
            if world.get::<MusicScoreNotesLayer>(child).is_some() {
                world.entity_mut(child).remove::<MusicScoreNotesLayer>().insert(FullScoreLayer {
                    grand,
                    root,
                    dirty: true,
                    end,
                });
            }
        }
    });
    root
}

pub(super) fn rebuild_full_score(
    mut commands: Commands,
    bravura: Option<Res<BravuraFont>>,
    tie: Option<Res<TieMaterialHandle>>,
    notes: Res<MusicScoreNotes>,
    meter: Res<MusicScoreMeter>,
    bars: Res<MusicScoreBarMap>,
    engraving: EngravingSettings,
    mut layers: Query<(Entity, &mut FullScoreLayer)>,
    visibility: Query<&InheritedVisibility>,
    existing: Query<(Entity, &ChildOf), With<MusicScoreNoteGlyph>>,
    mut clefs: ClefQuery,
    mut signatures: TimeSigQuery,
    mut roots: Query<(&mut Node, Option<&InstrumentScoreClef>, Option<&InstrumentOctaveAnnotation>), Without<MusicScoreClef>>,
) {
    let spacing = &engraving.spacing;
    let signature = &engraving.signature;
    let (Some(bravura), Some(tie)) = (bravura, tie) else { return };
    for (layer, mut full) in &mut layers {
        if notes.is_changed() || bars.is_changed() || meter.is_changed() || spacing.is_changed() || signature.is_changed() {
            full.dirty = true;
        }
        if !full.dirty || !visibility.get(full.root).is_ok_and(|v| v.get()) {
            continue;
        }
        full.dirty = false;
        let end = notes.0.iter().map(|n| n.start_beat + n.duration_beats).fold(full.end, f64::max);
        let forced = roots.get(full.root).ok().and_then(|(_, clef, _)| clef.map(|clef| clef.0));
        let octave_annotation = roots.get(full.root).ok().and_then(|(_, _, octave)| octave.map(|octave| octave.0)).unwrap_or(0);
        if let Ok((mut root, _, _)) = roots.get_mut(full.root) {
            root.width = Val::Px(score_beat_x(end, spacing.0) + 80.0);
            root.flex_shrink = 0.0;
        }
        spawn_window(
            &mut commands,
            &bravura,
            &tie,
            &notes.0,
            &meter,
            &bars,
            spacing.0,
            full.root,
            layer,
            0.0,
            -1.0,
            end + 1.0,
            &existing,
            &mut clefs,
            &mut signatures,
            full.grand,
            forced, octave_annotation, &signature.0,
        );
        commands.entity(layer).with_children(|parent| {
            for (index, beat) in std::iter::once(0.0).chain(bars.beats(0.0, end)).enumerate() {
                let number = index + usize::from(!bars.meter.has_pickup());
                parent.spawn((
                    MusicScoreNoteGlyph,
                    Text::new(number.to_string()),
                    TextFont { font_size: FontSize::Px(12.0), ..default() },
                    TextColor(Color::srgb(0.65, 0.72, 0.82)),
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(beat as f32 * spacing.0),
                        top: Val::Px(2.0),
                        ..default()
                    },
                    bevy::picking::Pickable::IGNORE,
                ));
            }
        });
    }
}

#[cfg(test)]
mod full_score_tests {
    use super::*;
    #[test]
    fn full_score_has_an_independent_layer_and_no_fixed_live_reference() {
        let mut app = App::new();
        app.world_mut().commands().spawn(Node::default()).with_children(|p| {
            let font = BravuraFont(Handle::default());
            spawn_music_score(p, &font);
            spawn_full_score(p, &font, true, 16.0);
        });
        app.world_mut().flush();
        let world = app.world_mut();
        assert_eq!(world.query::<&MusicScorePanel>().iter(world).count(), 1);
        assert_eq!(world.query::<&MusicScoreNotesLayer>().iter(world).count(), 1);
        assert_eq!(world.query::<&FullScoreLayer>().iter(world).count(), 1);
        assert_eq!(world.query::<&MusicScoreReference>().iter(world).count(), 1);
    }

    #[test]
    fn rebuilding_the_full_score_leaves_the_live_staffs_clef_alone() {
        let mut app = App::new();
        let (mut live, mut full) = (Entity::PLACEHOLDER, Entity::PLACEHOLDER);
        app.world_mut().commands().spawn(Node::default()).with_children(|p| {
            let font = BravuraFont(Handle::default());
            live = spawn_music_score(p, &font);
            full = spawn_full_score(p, &font, false, 4.0);
        });
        app.world_mut().flush();
        app.world_mut()
            .entity_mut(full)
            .insert((InstrumentScoreClef(Clef::Bass), InheritedVisibility::VISIBLE));
        app.insert_resource(BravuraFont(Handle::default()))
            .insert_resource(TieMaterialHandle {
                below: Handle::default(),
                above: Handle::default(),
                below_highlighted: Handle::default(),
                above_highlighted: Handle::default(),
            })
            .init_resource::<MusicScoreNotes>()
            .init_resource::<MusicScoreMeter>()
            .init_resource::<MusicScoreBarMap>()
            .init_resource::<MusicScoreSpacing>()
            .init_resource::<MusicScoreKeySignature>()
            .add_systems(Update, rebuild_full_score);
        app.update();

        let world = app.world_mut();
        let clef_of = |world: &mut World, root: Entity| {
            world
                .query_filtered::<(&Text, &ChildOf), With<MusicScoreClef>>()
                .iter(world)
                .find(|(_, parent)| parent.parent() == root)
                .map(|(text, _)| text.0.clone())
        };
        assert_eq!(clef_of(world, full).as_deref(), Some(Clef::Bass.glyph()));
        assert_eq!(clef_of(world, live).as_deref(), Some(Clef::Treble.glyph()));
    }
}

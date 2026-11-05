// SPDX-License-Identifier: MIT
//! Keep the trainer's playable range and fingering chart on the player's flute.
use super::*;
use harmonicon_core::instrument::flute::FootJoint;
use harmonicon_platform::settings::MyInstruments;
use harmonicon_ui::dialogs::combobox::{ComboboxSelect, spawn_combobox};

impl NoteTrainer {
    fn set_flute_foot(&mut self, foot: FootJoint) -> bool {
        let Some(flute) = self.flute() else { return false };
        if flute.foot == foot {
            return false;
        }
        let previous = self.practice.drill().copied();
        let auto_advance = self.practice.auto_advance;
        let landed = self.practice.landed;
        self.instrument = Flute::new(foot).into();
        self.practice = Practice::new(drills(&self.instrument, self.kind));
        self.practice.index = previous
            .and_then(|old| {
                self.practice.drills.iter().position(|d| d.position == old.position).or_else(|| {
                    self.practice
                        .drills
                        .iter()
                        .enumerate()
                        .min_by_key(|(_, d)| {
                            (d.target.round() as i16 - old.target.round() as i16).abs()
                        })
                        .map(|(i, _)| i)
                })
            })
            .unwrap_or(0);
        self.practice.auto_advance = auto_advance;
        self.practice.landed = landed;
        self.practice.restored = true;
        true
    }
}

pub(super) fn spawn(commands: &mut Commands, root: Entity, flute: &Flute, loc: &Localization) {
    let controls = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_shrink: 0.0,
            padding: UiRect::axes(Val::Px(24.0), Val::Px(8.0)),
            ..default()
        })
        .id();
    commands.entity(root).add_child(controls);
    let feet = [FootJoint::C, FootJoint::B];
    let labels = vec![loc.msg("flute-foot-c").to_string(), loc.msg("flute-foot-b").to_string()];
    spawn_combobox(
        commands,
        controls,
        root,
        &loc.msg("options-flute-foot"),
        &labels,
        &labels[usize::from(flute.foot == FootJoint::B)],
        move |ev: On<ComboboxSelect>,
              mut commands: Commands,
              mut trainer: ResMut<NoteTrainer>,
              mut mine: ResMut<MyInstruments>| {
            let Some(&foot) = feet.get(ev.index) else { return };
            mine.flute_foot = Some(foot);
            if trainer.set_flute_foot(foot) {
                commands.entity(root).despawn();
                // Rebuild both notation choices and physical keys from the same
                // instrument, including the B-foot roller and pitch detector range.
                commands.run_system_cached(layout::setup);
            }
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn changing_the_combobox_rebuilds_the_score_and_instrument_together() {
        use harmonicon_ui::dialogs::combobox::ComboboxValue;
        use harmonicon_ui::music_score::{BravuraFont, PitchChoiceInk};
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            bevy::asset::AssetPlugin::default(),
            bevy::scene::ScenePlugin,
        ));
        app.insert_resource(NoteTrainer::for_instrument(Flute::default().into()))
            .init_resource::<PitchRange>()
            .init_resource::<Localization>()
            .init_resource::<MyInstruments>()
            .insert_resource(BravuraFont(Handle::default()))
            .add_systems(Startup, layout::setup);
        app.update();
        let c_range = *app.world().resource::<PitchRange>();
        let combo = app
            .world_mut()
            .query_filtered::<Entity, With<ComboboxValue>>()
            .single(app.world())
            .unwrap();
        app.world_mut().trigger(ComboboxSelect { combobox: combo, value: String::new(), index: 1 });
        app.world_mut().flush();
        app.update();
        assert_eq!(app.world().resource::<MyInstruments>().flute_foot, Some(FootJoint::B));
        assert!(app.world().resource::<PitchRange>().min_freq < c_range.min_freq);
        assert_eq!(app.world().resource::<NoteTrainer>().flute().unwrap().foot, FootJoint::B);
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<GameplayRoot>>()
                .iter(app.world())
                .count(),
            1
        );
        assert!(
            app.world_mut().query::<&PitchChoiceInk>().iter(app.world()).any(|pitch| pitch.0 == 59)
        );
        assert_eq!(
            app.world_mut().query::<&FluteDrawing>().single(app.world()).unwrap().0.foot,
            FootJoint::B
        );
        let combo = app
            .world_mut()
            .query_filtered::<Entity, With<ComboboxValue>>()
            .single(app.world())
            .unwrap();
        app.world_mut().trigger(ComboboxSelect { combobox: combo, value: String::new(), index: 0 });
        app.world_mut().flush();
        app.update();
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<GameplayRoot>>()
                .iter(app.world())
                .count(),
            1
        );
        assert!(
            !app.world_mut()
                .query::<&PitchChoiceInk>()
                .iter(app.world())
                .any(|pitch| pitch.0 == 59)
        );
        assert_eq!(
            app.world_mut().query::<&FluteDrawing>().single(app.world()).unwrap().0.foot,
            FootJoint::C
        );
        assert_eq!(*app.world().resource::<PitchRange>(), c_range);
    }
    #[test]
    fn switching_foot_keeps_the_selected_pitch_and_practice_preferences() {
        let mut trainer = NoteTrainer::for_instrument(Flute::default().into());
        trainer.practice.index = 6;
        trainer.practice.auto_advance = false;
        trainer.practice.landed = 8;
        let target = *trainer.practice.drill().unwrap();
        assert!(trainer.set_flute_foot(FootJoint::B));
        assert_eq!(trainer.practice.drill().unwrap().position, target.position);
        assert_eq!(trainer.practice.drills.first().unwrap().target, 59.0);
        assert!(!trainer.practice.auto_advance);
        assert_eq!(trainer.practice.landed, 8);
        assert!(trainer.practice.restored);
        assert!(!trainer.set_flute_foot(FootJoint::B));
    }
    #[test]
    fn removing_low_b_selects_low_c_and_resets_the_attempt() {
        let mut trainer = NoteTrainer::for_instrument(Flute::new(FootJoint::B).into());
        assert_eq!(trainer.practice.drill().unwrap().target, 59.0);
        trainer.practice.held = 0.3;
        assert!(trainer.set_flute_foot(FootJoint::C));
        assert_eq!(trainer.practice.drill().unwrap().target, 60.0);
        assert!(trainer.practice.drills.iter().all(|d| d.target >= 60.0));
        assert_eq!(trainer.practice.held, 0.0);
    }
    #[test]
    fn switching_foot_preserves_octave_and_alternate_exercises() {
        for kind in [TechniqueKind::Register, TechniqueKind::Position] {
            let mut trainer = NoteTrainer::for_instrument(Flute::default().into());
            trainer.kind = kind;
            trainer.practice = Practice::new(drills(&trainer.instrument, kind));
            let position = trainer.practice.drill().unwrap().position;
            assert!(trainer.set_flute_foot(FootJoint::B));
            assert_eq!(trainer.kind, kind);
            assert_eq!(trainer.practice.drill().unwrap().position, position);
        }
    }
}

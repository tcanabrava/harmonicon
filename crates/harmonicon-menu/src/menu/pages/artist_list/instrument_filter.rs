// SPDX-License-Identifier: MIT

//! Instrument choices come from the catalog, independently of search/history.
use super::*;
use harmonicon_ui::dialogs::combobox;

#[derive(Component)]
pub(crate) struct InstrumentFilterHost {
    pub backdrop: Entity,
    pub choices: Vec<String>,
}

fn catalog_instruments(songs: &AvailableSongs) -> Vec<String> {
    songs
        .0
        .values()
        .flatten()
        .map(|song| song.instrument.clone())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub(crate) fn refresh_instrument_filter(
    mut commands: Commands,
    songs: Res<AvailableSongs>,
    loc: Res<Localization>,
    mut state: ResMut<SongPickerState>,
    mut hosts: Query<(Entity, &mut InstrumentFilterHost, Option<&Children>)>,
) {
    if !songs.is_changed()
        && !loc.is_changed()
        && !state.is_changed()
        && hosts.iter().all(|(_, _, children)| children.is_some())
    {
        return;
    }
    for (entity, mut host, children) in &mut hosts {
        let choices = catalog_instruments(&songs);
        if state.instrument_filter.as_ref().is_some_and(|id| !choices.contains(id)) {
            state.instrument_filter = None;
        }
        if host.choices == choices && children.is_some() && !loc.is_changed() {
            continue;
        }
        commands.entity(entity).despawn_children();
        commands.entity(host.backdrop).despawn_children();
        host.choices = choices.clone();
        let mut labels = vec![loc.msg("song-instrument-all").to_string()];
        labels.extend(choices.iter().map(|id| instrument_name(id, &loc)));
        let current = state.instrument_filter.as_ref();
        let selected = current
            .and_then(|id| choices.iter().position(|choice| choice == id))
            .map_or(0, |index| index + 1);
        combobox::spawn_combobox(
            &mut commands,
            entity,
            host.backdrop,
            &loc.msg("song-sort-instrument"),
            &labels,
            &labels[selected],
            move |ev: On<combobox::ComboboxSelect>, mut state: ResMut<SongPickerState>| {
                state.instrument_filter =
                    ev.index.checked_sub(1).and_then(|index| choices.get(index)).cloned();
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog() -> AvailableSongs {
        let mut songs = AvailableSongs::default();
        songs.0.insert(
            "artist".into(),
            ["piano", "guitar", "guitar", "harmonica"]
                .into_iter()
                .enumerate()
                .map(|(i, kind)| SongEntry {
                    instrument: kind.into(),
                    part: None,
                    artist: "Artist".into(),
                    name: format!("Song {i}"),
                    genre: "Rock".into(),
                    difficulty: "easy".into(),
                    source_name: String::new(),
                    retained: false,
                    asset_path: format!("song-{i}"),
                })
                .collect(),
        );
        songs
    }

    #[test]
    fn choices_include_only_catalog_identities_once() {
        assert_eq!(catalog_instruments(&catalog()), ["guitar", "harmonica", "piano"]);
        assert!(catalog_instruments(&AvailableSongs::default()).is_empty());
    }

    #[test]
    fn instrument_filter_composes_with_search() {
        let mut state = SongPickerState { instrument_filter: Some("guitar".into()), ..default() };
        let profile = PlayerProfile::default();
        assert_eq!(collect_songs(&catalog(), &state, &profile).len(), 2);
        state.query = "Song 2".into();
        assert_eq!(collect_songs(&catalog(), &state, &profile)[0].asset_path, "song-2");
    }

    #[test]
    fn catalog_changes_reset_an_unavailable_filter_and_rebuild_choices() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::scene::ScenePlugin))
            .insert_resource(catalog())
            .init_resource::<Localization>()
            .insert_resource(SongPickerState {
                instrument_filter: Some("flute".into()),
                ..default()
            })
            .add_systems(Update, refresh_instrument_filter);
        let backdrop = app.world_mut().spawn_empty().id();
        let host = app.world_mut().spawn(InstrumentFilterHost { backdrop, choices: vec![] }).id();
        app.update();
        assert!(app.world().resource::<SongPickerState>().instrument_filter.is_none());
        assert_eq!(app.world().get::<InstrumentFilterHost>(host).unwrap().choices.len(), 3);
        // Select by stable option index, never by translated display text.
        let combo = app
            .world_mut()
            .query_filtered::<Entity, With<combobox::ComboboxValue>>()
            .single(app.world())
            .unwrap();
        app.world_mut().trigger(combobox::ComboboxSelect {
            combobox: combo,
            index: 1,
            value: "translated label".into(),
        });
        assert_eq!(
            app.world().resource::<SongPickerState>().instrument_filter.as_deref(),
            Some("guitar")
        );
        assert_eq!(app.world().get::<Children>(backdrop).unwrap().len(), 1);
        app.world_mut().resource_mut::<AvailableSongs>().0.clear();
        app.update();
        assert!(app.world().get::<InstrumentFilterHost>(host).unwrap().choices.is_empty());
        assert!(app.world().resource::<SongPickerState>().instrument_filter.is_none());
        assert_eq!(app.world().get::<Children>(backdrop).unwrap().len(), 1);
    }
}

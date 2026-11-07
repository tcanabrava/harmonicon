// SPDX-License-Identifier: MIT
use super::*;
pub(crate) fn setup(
    mut commands: Commands,
    trainer: Res<NoteTrainer>,
    mut pitch_range: ResMut<PitchRange>,
    loc: Res<Localization>,
    font: Option<Res<harmonicon_ui::music_score::BravuraFont>>,
) {
    use harmonicon_ui::music_score::{
        MusicScoreBarMap, MusicScoreMeter, MusicScorePlayhead, MusicScoreSelection,
    };
    commands.insert_resource(MusicScoreBarMap::default());
    commands.insert_resource(MusicScoreMeter::default());
    commands.insert_resource(MusicScorePlayhead::default());
    commands.insert_resource(MusicScoreSelection::default());
    commands.insert_resource(harmonicon_ui::music_score::MusicScoreSections::default());
    if matches!(trainer.instrument, Instrument::Harmonica(_)) {
        return;
    }
    if let Some((low, high)) = trainer.instrument.range() {
        *pitch_range = PitchRange::from_freqs(
            [midi_to_freq_hz(f32::from(low)), midi_to_freq_hz(f32::from(high))],
            PITCH_RANGE_MARGIN_SEMITONES,
        );
    }
    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                min_height: Val::Px(0.0),
                max_height: Val::Percent(100.0),
                min_width: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(Color::srgb(0.05, 0.05, 0.08)),
            GameplayRoot,
        ))
        .id();
    let title = commands
        .spawn_scene(title_column_scene(
            loc.msg_args(
                "note-trainer-instrument-title",
                &[(
                    "instrument",
                    loc.msg(&format!("song-instrument-{}", trainer.instrument.kind().slug()))
                        .to_string(),
                )],
            )
            .to_string(),
        ))
        .id();
    let header = commands.spawn_scene(header_scene()).id();
    commands.entity(header).add_child(title);
    commands.entity(root).add_child(header);
    spawn_back_button(
        &mut commands,
        header,
        &loc.msg("back"),
        |_: On<Activate>,
         mut next: ResMut<NextState<AppState>>,
         mut ret_play: ResMut<harmonicon_app::app::ReturnToPlay>| {
            ret_play.0 = true;
            next.set(AppState::Menu);
        },
    );

    let tabs = commands
        .spawn(Node { padding: UiRect::axes(Val::Px(24.0), Val::Px(8.0)), ..default() })
        .id();
    commands.entity(root).add_child(tabs);
    let kinds = trainer.kinds();
    let mut labels: Vec<String> =
        kinds.iter().map(|&k| String::from(loc.msg(kind_key(&trainer.instrument, k)))).collect();
    let mut selected = kinds.iter().position(|&k| k == trainer.kind).unwrap_or(0);
    if trainer.offers_chords() {
        labels.insert(1, loc.msg("trainer-chords-tab").to_string());
        selected = if trainer.chord.is_some() {
            1
        } else if selected > 0 {
            selected + 1
        } else {
            0
        };
    }
    spawn_tab_bar(
        &mut commands,
        tabs,
        &labels,
        selected,
        |ev: On<TabSelect>,
         mut trainer: ResMut<NoteTrainer>,
         mut profile: ResMut<harmonicon_app::profile::PlayerProfile>| {
            let mut index = ev.index;
            if trainer.offers_chords() {
                if index == 1 {
                    if trainer.remember(&mut profile) {
                        harmonicon_app::profile::save_profile(&profile);
                    }
                    let shapes =
                        trainer.guitar().and_then(chords::shapes_for).unwrap_or(chords::SHAPES);
                    trainer.chord = Some(chords::ChordPractice::new(shapes));
                    trainer.practice.auto_advance = false;
                    return;
                }
                if index > 1 {
                    index -= 1;
                }
            }
            if let Some(&kind) = trainer.kinds().get(index)
                && trainer.set_kind(kind, &mut profile)
            {
                harmonicon_app::profile::save_profile(&profile);
            }
        },
    );

    if let Some(flute) = trainer.flute() {
        flute_setup::spawn(&mut commands, root, flute, &loc);
    }

    if let Instrument::Clarinet(setup) = trainer.instrument {
        clarinet::spawn_setup(&mut commands, root, setup, &loc);
    }
    if matches!(trainer.instrument, Instrument::Bowed(_)) {
        commands.entity(root).with_child((
            bowed::StringChoices,
            Node { padding: UiRect::all(Val::Px(12.0)), ..default() },
        ));
    }
    let previous = String::from(loc.msg("note-trainer-previous"));
    let next = String::from(loc.msg("note-trainer-next"));
    let listen = String::from(loc.msg("note-trainer-listen"));
    if trainer.guitar().is_some() {
        commands.entity(root).with_children(|p| {
            p.spawn((
                selection::Controls,
                Node {
                    width: Val::Percent(100.0),
                    flex_wrap: FlexWrap::Wrap,
                    column_gap: Val::Px(16.0),
                    row_gap: Val::Px(8.0),
                    padding: UiRect::all(Val::Px(16.0)),
                    flex_shrink: 0.0,
                    ..default()
                },
            ));
            p.spawn((
                selection::Backdrops,
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                bevy::picking::Pickable::IGNORE,
            ));
        });
    }
    let mut content = root;
    commands.entity(root).with_children(|parent| {
        content = harmonicon_ui::dialogs::scroll_area::spawn_scroll_area(
            parent,
            Color::srgb(0.4, 0.55, 0.7),
            Color::srgb(0.1, 0.12, 0.16),
        );
    });
    commands.entity(content).insert(Node {
        width: Val::Percent(100.0),
        flex_grow: 1.0,
        flex_basis: Val::Px(0.0),
        min_width: Val::Px(0.0),
        min_height: Val::Px(0.0),
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Stretch,
        overflow: Overflow::scroll_y(),
        row_gap: Val::Px(16.0),
        ..default()
    });
    commands.entity(content).with_children(|root| {
        root.spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: if trainer.guitar().is_some()
                || trainer.instrument.fingering_chart().is_some()
            {
                FlexDirection::Row
            } else {
                FlexDirection::Column
            },
            flex_wrap: FlexWrap::Wrap,
            column_gap: Val::Px(16.0),
            align_items: AlignItems::Center,
            flex_shrink: 0.0,
            ..default()
        })
        .with_children(|workspace| {
            workspace
                .spawn(Node {
                    width: Val::Percent(100.0),
                    flex_basis: if trainer.guitar().is_some()
                        || matches!(trainer.instrument, Instrument::Clarinet(_))
                    {
                        Val::Px(420.0)
                    } else if trainer.flute().is_some() {
                        Val::Px(300.0)
                    } else {
                        Val::Auto
                    },
                    flex_grow: 1.0,
                    min_width: Val::Px(0.0),
                    flex_shrink: 0.0,
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(12.0),
                    padding: UiRect::axes(Val::Px(24.0), Val::Px(12.0)),
                    ..default()
                })
                .with_children(|card| {
                    card.spawn((
                        InstructionLabel,
                        Text::new(""),
                        TextFont { font_size: FontSize::Px(28.0), ..default() },
                        TextColor(Color::WHITE),
                    ));
                    card.spawn((
                        StatusLabel,
                        Text::new(""),
                        TextFont { font_size: FontSize::Px(22.0), ..default() },
                        TextColor(Color::srgb(0.75, 0.85, 1.0)),
                    ));
                    if trainer.guitar().is_some() {
                        bend::spawn(card, &loc);
                        chords::spawn(card, &loc);
                    }
                    card.spawn((
                        Node {
                            width: Val::Percent(100.0),
                            max_width: Val::Px(320.0),
                            height: Val::Px(10.0),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.12, 0.13, 0.18)),
                    ))
                    .with_children(|track| {
                        track.spawn((
                            HoldBar,
                            Node {
                                width: Val::Percent(0.0),
                                height: Val::Percent(100.0),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.35, 0.85, 0.45)),
                        ));
                    });
                    card.spawn(Node { column_gap: Val::Px(8.0), ..default() }).with_children(
                        |row| {
                            row.spawn_empty().apply_scene(button::small(
                                &previous,
                                |_: On<Activate>, mut t: ResMut<NoteTrainer>| {
                                    t.step_by(-1);
                                },
                            ));
                            row.spawn_empty().apply_scene(button::small(&listen, play_reference));
                            row.spawn_empty().apply_scene(button::small(
                                &next,
                                |_: On<Activate>, mut t: ResMut<NoteTrainer>| {
                                    t.step_by(1);
                                },
                            ));
                        },
                    );
                    card.spawn((
                        LandedLabel,
                        Text::new(""),
                        TextFont { font_size: FontSize::Px(16.0), ..default() },
                        TextColor(Color::srgb(0.6, 0.65, 0.75)),
                    ));
                    if let Instrument::Clarinet(setup) = trainer.instrument {
                        super::super::wind_visuals::spawn_slot(card, setup, false);
                        card.spawn((
                            Text::new(loc.msg("clarinet-diagram-help").to_string()),
                            TextFont { font_size: FontSize::Px(13.0), ..default() },
                        ));
                    }
                });
            if let Some(font) = font.as_deref() {
                workspace
                    .spawn(Node {
                        width: Val::Percent(100.0),
                        flex_basis: if trainer.guitar().is_some() {
                            Val::Px(400.0)
                        } else if trainer.instrument.fingering_chart().is_some() {
                            Val::Px(680.0)
                        } else {
                            Val::Auto
                        },
                        flex_grow: 1.0,
                        min_width: Val::Px(0.0),
                        flex_wrap: FlexWrap::Wrap,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(32.0),
                        padding: UiRect::horizontal(Val::Px(16.0)),
                        ..default()
                    })
                    .with_children(|score| {
                        if let Some(setup) = trainer.guitar().filter(|_| trainer.offers_chords()) {
                            chords::spawn_diagram(score, setup);
                        }
                        score
                            .spawn(Node {
                                width: Val::Percent(100.0),
                                flex_basis: Val::Px(330.0),
                                flex_grow: 1.0,
                                min_width: Val::Px(0.0),
                                ..default()
                            })
                            .with_children(|staff| {
                                if trainer.instrument.adapter().score_picker() {
                                    pitch_picker::spawn_for_instrument(
                                        staff,
                                        font,
                                        &trainer.instrument,
                                        &loc,
                                    );
                                } else {
                                    teaching::spawn_staff(
                                        staff,
                                        font,
                                        trainer.piano().is_some(),
                                        Some(trainer.instrument.kind()),
                                        &loc,
                                    );
                                }
                            });
                    });
            }
        });
        if matches!(trainer.instrument, Instrument::Clarinet(_)) {
            return;
        }
        root.spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            flex_shrink: 0.0,
            min_height: Val::Px(if trainer.flute().is_some() {
                156.0
            } else if trainer.piano().is_some() {
                250.0
            } else {
                320.0
            }),
            padding: UiRect::axes(Val::Px(24.0), Val::Px(12.0)),
            ..default()
        })
        .with_children(|strip| {
            match (trainer.guitar(), trainer.flute(), trainer.piano()) {
                (Some(guitar), _, _) => {
                    let board = spawn_trainer_neck(strip, guitar, &loc);
                    selection::make_board_selectable(&mut strip.commands(), board);
                    strip.spawn((
                        chords::BoardHelp,
                        Text::new(loc.msg("trainer-board-help").to_string()),
                        TextFont { font_size: FontSize::Px(15.0), ..default() },
                        TextColor(Color::WHITE),
                    ));
                    strip
                        .spawn((
                            PositionHint,
                            Node::default(),
                            harmonicon_ui::dialogs::drawer::Drawer { open: false },
                        ))
                        .with_children(|hint| {
                            hint.spawn_empty().apply_scene(button::small(
                                &loc.msg("note-trainer-show-position"),
                                |_: On<Activate>, mut trainer: ResMut<NoteTrainer>| {
                                    trainer.practice.revealed = true
                                },
                            ));
                        });
                }
                (_, Some(flute), _) => {
                    use super::super::flute_visuals::{ChartAxis, ChartSlot, spawn_flute_chart};
                    strip
                        .spawn(Node {
                            width: Val::Percent(100.0),
                            max_width: Val::Px(1080.0),
                            ..default()
                        })
                        .with_children(|drawing| {
                            spawn_flute_chart(
                                drawing,
                                flute,
                                1.25,
                                ChartSlot::Now,
                                ChartAxis::Horizontal,
                            )
                        });
                    strip.spawn((
                        Text::new(loc.msg("note-trainer-flute-key-help").to_string()),
                        TextFont { font_size: FontSize::Px(14.0), ..default() },
                        TextColor(Color::WHITE),
                    ));
                }
                (_, _, Some(piano)) => piano::spawn(strip, piano, &loc),
                _ => {
                    if let Instrument::Bowed(setup) = &trainer.instrument {
                        super::super::bowed_visuals::spawn(strip, setup, &loc);
                    }
                    if let Instrument::Clarinet(setup) = trainer.instrument {
                        super::super::wind_visuals::spawn_slot(strip, setup, false);
                        strip.spawn((
                            Text::new(loc.msg("clarinet-diagram-help").to_string()),
                            TextFont { font_size: FontSize::Px(14.0), ..default() },
                        ));
                    }
                }
            }
        });
    });
}

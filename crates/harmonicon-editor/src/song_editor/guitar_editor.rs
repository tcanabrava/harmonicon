// SPDX-License-Identifier: MIT

//! Guitar tablature view. Every edit uses core positions and voice rules;
//! no guitar coordinate is stored as a harmonica hole.
use super::TICKS_PER_BEAT;
use super::state::{EditorState, Expr, GridNote};
use bevy::input_focus::tab_navigation::{TabGroup, TabIndex};
use bevy::prelude::*;
use bevy::ui_widgets::{Activate, Button};
use harmonicon_app::app::AppState;
use harmonicon_core::instrument::guitar::{FretAssignment, StringTechnique};
use harmonicon_core::instrument::{Instrument, InstrumentModel, PlayPosition};
use harmonicon_core::note_target::NoteTarget;
use harmonicon_platform::localization::{Localization, LocalizationExt};
use harmonicon_ui::dialogs::file_dialog::{DialogMode, OpenFileDialog};

#[derive(Component)]
pub(super) struct GuitarRoot;
#[derive(Resource)]
pub(super) struct GuitarControls {
    string: u8,
    tick: usize,
    fret: u8,
    len: usize,
    invalid: bool,
}
impl Default for GuitarControls {
    fn default() -> Self {
        Self { string: 1, tick: 0, fret: 0, len: TICKS_PER_BEAT, invalid: false }
    }
}
#[derive(Component, Clone, Copy)]
pub(super) enum Action {
    Select(u32),
    Fret(i8),
    String(i8),
    Length(i8),
    Time(i8),
    Insert,
    Delete,
    Save,
    Load,
    Play,
    Stop,
    Back,
    Undo,
    Redo,
    Bend,
    Harmonic,
}

pub(super) fn is_guitar(state: Option<Res<EditorState>>) -> bool {
    state.is_some_and(|s| matches!(s.effective_instrument(), Instrument::Guitar(_)))
}
pub(super) fn is_harmonica(state: Option<Res<EditorState>>) -> bool {
    state.is_none_or(|s| matches!(s.effective_instrument(), Instrument::Harmonica(_)))
}

fn button(parent: &mut ChildSpawnerCommands, label: impl Into<String>, action: Action, width: f32) {
    colored_button(parent, label, action, width, Color::srgb(0.15, 0.19, 0.26));
}

/// A note on the timeline: a button exactly `width` × `height`, in its
/// string's `color`, so its length on screen is its length in time.
pub(super) fn note_block_button(
    parent: &mut ChildSpawnerCommands,
    label: impl Into<String>,
    action: Action,
    width: f32,
    height: f32,
    color: Color,
) {
    parent
        .spawn((
            Button,
            TabIndex(0),
            action,
            Node {
                width: Val::Px(width),
                height: Val::Px(height),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(color),
            children![(
                Text::new(label),
                TextFont { font_size: FontSize::Px(14.0), ..default() },
                TextColor(Color::srgb(0.05, 0.05, 0.08))
            )],
        ))
        .observe(handle_action);
}

/// Moves the cursor to the string and snapped tick under a click on a lane,
/// and drops the selection so the next Add note goes there.
// not-a-widget-button: a lane is a continuous surface whose click position
// is the input; keyboard users move the cursor with S, ‹ and › instead.
pub(super) fn on_lane_click(
    ev: On<bevy::picking::events::PointerClick>,
    lanes: Query<(&super::guitar_timeline::Lane, &bevy::ui::RelativeCursorPosition)>,
    mut state: ResMut<EditorState>,
    mut controls: ResMut<GuitarControls>,
) {
    let Ok((lane, cursor)) = lanes.get(ev.entity) else { return };
    let Some(normalized) = cursor.normalized else { return };
    // `normalized` runs -0.5..0.5 across the node.
    let tick =
        super::guitar_timeline::click_tick(normalized.x + 0.5, lane.total_ticks, state.snap_mode);
    controls.string = lane.string;
    controls.tick = tick;
    state.selected.clear();
}

/// [`button`] in `color`: a note takes its string's colour.
fn colored_button(
    parent: &mut ChildSpawnerCommands,
    label: impl Into<String>,
    action: Action,
    width: f32,
    color: Color,
) {
    parent
        .spawn((
            Button,
            TabIndex(0),
            action,
            Node {
                width: Val::Px(width),
                min_height: Val::Px(30.0),
                padding: UiRect::all(Val::Px(5.0)),
                margin: UiRect::all(Val::Px(2.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(color),
            children![(
                Text::new(label),
                TextFont { font_size: FontSize::Px(14.0), ..default() },
                TextColor(Color::WHITE)
            )],
        ))
        .observe(handle_action);
}

pub(super) fn setup(
    mut commands: Commands,
    state: Res<EditorState>,
    controls: Res<GuitarControls>,
    timeline_scroll: Res<super::guitar_timeline::TimelineScroll>,
    loc: Res<Localization>,
) {
    let Instrument::Guitar(guitar) = state.effective_instrument() else { return };
    let mut area = Entity::PLACEHOLDER;
    commands
        .spawn((
            GuitarRoot,
            TabGroup::default(),
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(12.0)),
                row_gap: Val::Px(8.0),
                overflow: Overflow::scroll(),
                ..default()
            },
            BackgroundColor(Color::srgb(0.04, 0.05, 0.08)),
        ))
        .with_children(|root| {
            area = harmonicon_ui::dialogs::scroll_area::spawn_scroll_area_xy(
                root,
                Color::srgb(0.5, 0.6, 0.8),
                Color::srgb(0.1, 0.12, 0.18),
            );
        });
    commands.entity(area).with_children(|scroll| {
        scroll
            .spawn(Node {
                flex_direction: FlexDirection::Column,
                width: Val::Percent(100.0),
                min_width: Val::Px(600.0),
                row_gap: Val::Px(8.0),
                ..default()
            })
            .with_children(|root| {
                root.spawn((
                    Text::new(if state.name.is_empty() {
                        loc.msg("editor-guitar-tab").to_string()
                    } else {
                        format!("{} — {}", state.name, loc.msg("editor-guitar-tab"))
                    }),
                    TextColor(Color::WHITE),
                ));
                root.spawn(Node { flex_wrap: FlexWrap::Wrap, ..default() }).with_children(|row| {
                    for (key, action) in [
                        ("editor-back-label", Action::Back),
                        ("editor-guitar-load", Action::Load),
                        ("editor-guitar-save", Action::Save),
                        ("editor-guitar-play", Action::Play),
                        ("editor-guitar-stop", Action::Stop),
                        ("editor-guitar-undo", Action::Undo),
                        ("editor-guitar-redo", Action::Redo),
                    ] {
                        button(row, loc.msg(key).to_string(), action, 100.0);
                    }
                });
                root.spawn(Node {
                    flex_wrap: FlexWrap::Wrap,
                    column_gap: Val::Px(12.0),
                    row_gap: Val::Px(4.0),
                    ..default()
                })
                .with_children(|row| {
                    for (field, key) in [
                        (super::state::Field::Name, "editor-field-name"),
                        (super::state::Field::Author, "editor-field-author"),
                        (super::state::Field::Tempo, "editor-field-tempo"),
                        (super::state::Field::Key, "editor-guitar-key"),
                    ] {
                        row.spawn(Node { flex_direction: FlexDirection::Column, ..default() })
                            .with_children(|cell| {
                                cell.spawn((
                                    Text::new(loc.msg(key).to_string()),
                                    TextColor(Color::WHITE),
                                ));
                                let parent = cell.target_entity();
                                harmonicon_ui::dialogs::text_input::spawn_text_input(
                            cell.commands_mut(),
                            parent,
                            state.field_text(field),
                            150.0,
                            Color::srgb(0.08, 0.1, 0.15),
                            Color::srgb(0.3, 0.35, 0.45),
                            move |ev: On<
                                harmonicon_ui::dialogs::text_input::TextInputCommitted,
                            >,
                                  mut state: ResMut<EditorState>| {
                                if state.field_text(field) != ev.value {
                                    state.field_text_mut(field).clone_from(&ev.value);
                                }
                            },
                        );
                            });
                    }
                });
                root.spawn((
                    Text::new(
                        loc.msg_args(
                            "editor-guitar-target",
                            &[
                                ("string", controls.string.to_string()),
                                ("fret", controls.fret.to_string()),
                                ("tick", controls.tick.to_string()),
                                ("length", controls.len.to_string()),
                            ],
                        )
                        .to_string(),
                    ),
                    TextColor(Color::WHITE),
                ));
                root.spawn(Node { flex_wrap: FlexWrap::Wrap, ..default() }).with_children(|row| {
                    for (label, action) in [
                        ("S −", Action::String(-1)),
                        ("S +", Action::String(1)),
                        ("F −", Action::Fret(-1)),
                        ("F +", Action::Fret(1)),
                        // `‹`/`›` nudge time; `←`/`→` below turn the page.
                        // Two pairs of identical arrows would read the same
                        // to a screen reader for two different actions.
                        ("‹", Action::Time(-1)),
                        ("›", Action::Time(1)),
                        ("−", Action::Length(-1)),
                        ("+", Action::Length(1)),
                    ] {
                        button(row, label, action, 45.0);
                    }
                    for (key, action) in [
                        ("editor-guitar-add", Action::Insert),
                        ("editor-guitar-delete", Action::Delete),
                        ("editor-guitar-bend", Action::Bend),
                        ("editor-guitar-harmonic", Action::Harmonic),
                    ] {
                        button(row, loc.msg(key).to_string(), action, 100.0);
                    }
                });
                root.spawn((
                    Text::new(loc.msg("editor-guitar-help").to_string()),
                    TextFont { font_size: FontSize::Px(14.0), ..default() },
                    Node {
                        width: Val::Percent(100.0),
                        max_width: Val::Percent(100.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.7, 0.8, 0.9)),
                ));
                root.spawn((Text::new(""), TextColor(Color::WHITE), FeedbackLabel));
                if controls.invalid {
                    root.spawn((
                        Text::new(loc.msg("editor-guitar-invalid").to_string()),
                        TextColor(Color::srgb(1.0, 0.4, 0.3)),
                    ));
                }
                super::guitar_timeline::spawn(root, &state, &guitar, timeline_scroll.0);
            });
    });
}

/// Test overlap against the actual time intervals, including already-held
/// strings. Only the edited note changes; invalid changes leave it intact.
fn valid(state: &EditorState, candidate: &GridNote) -> bool {
    let instrument = state.effective_instrument();
    let Some(position) = candidate.target.position() else { return false };
    if candidate.target.with_placement(&instrument, position).is_err() {
        return false;
    }
    state
        .notes
        .iter()
        .filter(|n| {
            n.id != candidate.id
                && n.tick < candidate.tick.saturating_add(candidate.len)
                && candidate.tick < n.tick.saturating_add(n.len)
        })
        .all(|n| n.target.position().is_some_and(|p| instrument.can_sound_together(&position, &p)))
}

fn handle_action(
    ev: On<Activate>,
    actions: Query<&Action>,
    mut state: ResMut<EditorState>,
    mut controls: ResMut<GuitarControls>,
    mut commands: Commands,
    loc: Res<Localization>,
    mut sources: ResMut<Assets<AudioSource>>,
    audio: Res<harmonicon_audio::AudioSettings>,
    playing: Query<Entity, With<super::playback::EditorAudio>>,
    mut playhead: ResMut<super::playback::Playhead>,
    mut next: ResMut<NextState<AppState>>,
) {
    let Ok(action) = actions.get(ev.entity) else { return };
    controls.invalid = false;
    match *action {
        Action::Back => {
            next.set(AppState::Menu);
            return;
        }
        Action::Load | Action::Save => {
            let save = matches!(action, Action::Save);
            commands.write_message(OpenFileDialog {
                purpose: if save { super::SAVE_PURPOSE } else { super::LOAD_PURPOSE },
                title: loc
                    .msg(if save { "editor-guitar-save" } else { "editor-guitar-load" })
                    .to_string(),
                extensions: vec!["harpchart".into()],
                start_dir: None,
                mode: if save {
                    DialogMode::Save { default_name: "guitar.harpchart".into() }
                } else {
                    DialogMode::Open
                },
            });
            return;
        }
        Action::Play => {
            super::playback::start_playback(
                &state,
                &mut sources,
                &audio,
                &playing,
                &mut playhead,
                &mut commands,
            );
            return;
        }
        Action::Stop => {
            for e in &playing {
                commands.entity(e).despawn();
            }
            *playhead = default();
            return;
        }
        Action::Undo => {
            commands.write_message(super::state::DocumentCommand::Undo);
            return;
        }
        Action::Redo => {
            commands.write_message(super::state::DocumentCommand::Redo);
            return;
        }
        Action::Select(id) => {
            if let Some(note) = state.notes.iter().find(|n| n.id == id)
                && let Some(PlayPosition::Fret(at)) = note.target.position()
            {
                controls.string = at.string;
                controls.fret = at.fret;
                controls.tick = note.tick;
                controls.len = note.len;
                state.select_only(id);
            }
            return;
        }
        Action::Delete => {
            let ids = state.selected.clone();
            state.notes.retain(|n| !ids.contains(&n.id));
            state.prune_selection();
            return;
        }
        Action::String(_)
        | Action::Fret(_)
        | Action::Length(_)
        | Action::Time(_)
        | Action::Insert
        | Action::Bend
        | Action::Harmonic => {}
    }
    if !apply_note_edit(&mut state, &mut controls, *action) {
        controls.invalid = true;
    }
}

/// The next bend depth a press of Bend gives a note at `technique`: off, a
/// half step, a whole step (the guitar's most common bend), then off again.
/// A harmonic can't be bent, so a press on one starts from a half step.
fn next_bend(technique: StringTechnique) -> StringTechnique {
    match technique {
        StringTechnique::Bend(depth) if depth < 1.5 => StringTechnique::Bend(2.0),
        StringTechnique::Bend(_) => StringTechnique::Fretted,
        StringTechnique::Fretted | StringTechnique::Harmonic => StringTechnique::Bend(1.0),
    }
}

/// Applies one note-shaping control to the selected note, or adds a note
/// for Insert. Moves the controls first (string, fret, time, length), then
/// builds the note they describe and keeps it only if the guitar can play
/// it alongside everything sounding with it. Returns `false` for an edit
/// the guitar rejects, which leaves every note as it was. With no note
/// selected only Insert does anything; the other controls just move.
fn apply_note_edit(state: &mut EditorState, controls: &mut GuitarControls, action: Action) -> bool {
    let instrument = state.effective_instrument();
    let Instrument::Guitar(guitar) = &instrument else { return true };
    match action {
        Action::String(delta) => {
            controls.string =
                controls.string.saturating_add_signed(delta).clamp(1, guitar.string_count());
        }
        Action::Fret(delta) => controls.fret = controls.fret.saturating_add_signed(delta),
        Action::Length(delta) => {
            controls.len = controls.len.saturating_add_signed(isize::from(delta)).max(1);
        }
        Action::Time(delta) => {
            controls.tick = controls.tick.saturating_add_signed(isize::from(delta));
        }
        Action::Insert | Action::Bend | Action::Harmonic => {}
        _ => return true,
    }
    let selected =
        state.selected.first().and_then(|id| state.notes.iter().find(|n| n.id == *id)).copied();
    if selected.is_none() && !matches!(action, Action::Insert) {
        return true;
    }
    let mut at = selected
        .and_then(|n| n.target.position())
        .and_then(|p| if let PlayPosition::Fret(at) = p { Some(at) } else { None })
        .unwrap_or(FretAssignment {
            string: controls.string,
            fret: controls.fret,
            technique: StringTechnique::Fretted,
        });
    at.fret = controls.fret;
    if matches!(action, Action::String(_)) {
        // Moving a note to another string keeps what it sounds: find the
        // fret there that plays the same pitch, or refuse.
        if let Some(note) = selected {
            let Some(pitch) = note.target.sounding_pitch(&instrument) else { return false };
            let Some(fret) = (0..=guitar.top_fret()).find(|fret| {
                let candidate = PlayPosition::Fret(FretAssignment {
                    string: controls.string,
                    fret: *fret,
                    technique: at.technique,
                });
                NoteTarget::Positioned(candidate)
                    .sounding_pitch(&instrument)
                    .is_some_and(|p| (p - pitch).abs() < 0.001)
            }) else {
                return false;
            };
            at.fret = fret;
            controls.fret = fret;
        }
        at.string = controls.string;
    }
    at.technique = match action {
        Action::Bend => next_bend(at.technique),
        Action::Harmonic if at.technique == StringTechnique::Harmonic => StringTechnique::Fretted,
        Action::Harmonic => StringTechnique::Harmonic,
        _ => at.technique,
    };
    let candidate = GridNote {
        id: selected.map_or(state.next_id, |n| n.id),
        tick: controls.tick,
        len: controls.len,
        expr: selected.map_or(Expr::None, |n| n.expr),
        target: NoteTarget::Positioned(PlayPosition::Fret(at)),
    };
    if !valid(state, &candidate) {
        return false;
    }
    if let Some(note) = state.notes.iter_mut().find(|n| n.id == candidate.id) {
        *note = candidate;
    } else {
        state.notes.push(candidate);
        state.next_id += 1;
    }
    state.select_only(candidate.id);
    true
}

pub(super) fn refresh(
    mut commands: Commands,
    state: Res<EditorState>,
    controls: Res<GuitarControls>,
    loc: Res<Localization>,
    roots: Query<Entity, With<GuitarRoot>>,
) {
    if state.is_changed() || controls.is_changed() || loc.is_changed() || roots.is_empty() {
        for root in &roots {
            commands.entity(root).despawn();
        }
        commands.run_system_cached(setup);
    }
}

pub(super) fn spawn_selector(parent: &mut ChildSpawnerCommands, loc: &Localization) {
    parent
        .spawn((
            Button,
            Node { padding: UiRect::all(Val::Px(8.0)), ..default() },
            BackgroundColor(Color::srgb(0.15, 0.19, 0.26)),
            children![(
                Text::new(loc.msg("editor-guitar-select").to_string()),
                TextColor(Color::WHITE)
            )],
        ))
        .observe(
            |_: On<Activate>,
             state: Res<EditorState>,
             mut commands: Commands,
             mut feedback: ResMut<super::save_feedback::SaveFeedback>,
             loc: Res<Localization>| {
                if !state.notes.is_empty() || state.mode != super::state::Mode::Edit {
                    feedback.set(loc.msg("editor-guitar-select-empty"));
                    return;
                }
                commands.write_message(super::state::DocumentCommand::SelectInstrument(
                    super::state::InstrumentSelection::Configured {
                        instrument: harmonicon_core::instrument::guitar::Guitar::standard(
                            harmonicon_core::instrument::guitar::GuitarBody::Electric,
                        )
                        .into(),
                        harmonica_origin: None,
                    },
                ));
            },
        );
}

#[derive(Component)]
pub(super) struct FeedbackLabel;
pub(super) fn update_feedback(
    feedback: Res<super::save_feedback::SaveFeedback>,
    mut labels: Query<(&mut Text, &mut Node), With<FeedbackLabel>>,
) {
    let text = feedback.current().map_or(String::new(), ToString::to_string);
    for (mut label, mut node) in &mut labels {
        // Font fallback may have created child spans; hide their parent as
        // well when feedback expires so stale spans cannot remain visible.
        node.display = if feedback.current().is_some() { Display::Flex } else { Display::None };
        if label.0 != text {
            label.0.clone_from(&text);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use harmonicon_core::instrument::guitar::{Guitar, GuitarBody};

    fn guitar_state() -> EditorState {
        EditorState {
            instrument: super::super::state::InstrumentSelection::Configured {
                instrument: Guitar::standard(GuitarBody::Electric).into(),
                harmonica_origin: None,
            },
            ..default()
        }
    }

    fn at(state: &EditorState, id: u32) -> FretAssignment {
        let note = state.notes.iter().find(|n| n.id == id).expect("note exists");
        match note.target.position() {
            Some(PlayPosition::Fret(at)) => at,
            other => panic!("not a string position: {other:?}"),
        }
    }

    /// A3 placed on the third string's second fret, selected.
    fn with_a3() -> (EditorState, GuitarControls) {
        let mut state = guitar_state();
        let mut controls = GuitarControls { string: 3, fret: 2, ..default() };
        assert!(apply_note_edit(&mut state, &mut controls, Action::Insert));
        (state, controls)
    }

    #[test]
    fn insert_places_a_note_where_the_controls_point_and_selects_it() {
        let (state, _) = with_a3();
        assert_eq!(state.notes.len(), 1);
        assert_eq!(
            at(&state, 0),
            FretAssignment { string: 3, fret: 2, technique: StringTechnique::Fretted }
        );
        assert_eq!(state.selected, [0]);
        assert_eq!(state.next_id, 1);
    }

    #[test]
    fn fret_time_and_length_controls_reshape_the_selected_note() {
        let (mut state, mut controls) = with_a3();
        assert!(apply_note_edit(&mut state, &mut controls, Action::Fret(1)));
        assert!(apply_note_edit(&mut state, &mut controls, Action::Time(3)));
        assert!(apply_note_edit(&mut state, &mut controls, Action::Length(-2)));
        assert_eq!(at(&state, 0).fret, 3);
        let note = state.notes[0];
        assert_eq!((note.tick, note.len), (3, TICKS_PER_BEAT - 2));
    }

    /// Moving to another string keeps the note's sound: A3 on string 4 is
    /// the seventh fret.
    #[test]
    fn a_string_change_finds_the_fret_that_keeps_the_pitch() {
        let (mut state, mut controls) = with_a3();
        assert!(apply_note_edit(&mut state, &mut controls, Action::String(1)));
        assert_eq!(
            at(&state, 0),
            FretAssignment { string: 4, fret: 7, technique: StringTechnique::Fretted }
        );
        assert_eq!(controls.fret, 7, "the controls follow the note");
    }

    /// A3 is below the second string's open B3: nowhere to go, nothing moves.
    #[test]
    fn a_string_with_no_equivalent_fret_refuses_and_leaves_the_note() {
        let (mut state, mut controls) = with_a3();
        assert!(!apply_note_edit(&mut state, &mut controls, Action::String(-1)));
        assert_eq!(
            at(&state, 0),
            FretAssignment { string: 3, fret: 2, technique: StringTechnique::Fretted }
        );
    }

    #[test]
    fn bend_cycles_through_a_half_step_and_a_whole_step_to_off() {
        let (mut state, mut controls) = with_a3();
        let mut techniques = Vec::new();
        for _ in 0..3 {
            assert!(apply_note_edit(&mut state, &mut controls, Action::Bend));
            techniques.push(at(&state, 0).technique);
        }
        assert_eq!(
            techniques,
            [StringTechnique::Bend(1.0), StringTechnique::Bend(2.0), StringTechnique::Fretted]
        );
    }

    #[test]
    fn harmonic_toggles_where_the_fret_has_one() {
        let mut state = guitar_state();
        let mut controls = GuitarControls { string: 6, fret: 12, ..default() };
        assert!(apply_note_edit(&mut state, &mut controls, Action::Insert));
        assert!(apply_note_edit(&mut state, &mut controls, Action::Harmonic));
        assert_eq!(at(&state, 0).technique, StringTechnique::Harmonic);
        assert!(apply_note_edit(&mut state, &mut controls, Action::Harmonic));
        assert_eq!(at(&state, 0).technique, StringTechnique::Fretted);
        // No natural harmonic sits over the second fret.
        let (mut state, mut controls) = with_a3();
        assert!(!apply_note_edit(&mut state, &mut controls, Action::Harmonic));
        assert_eq!(at(&state, 0).technique, StringTechnique::Fretted);
    }

    #[test]
    fn an_insert_onto_a_sounding_string_is_refused() {
        let (mut state, mut controls) = with_a3();
        state.selected.clear();
        controls.fret = 4;
        assert!(!apply_note_edit(&mut state, &mut controls, Action::Insert));
        assert_eq!(state.notes.len(), 1);
    }

    #[test]
    fn with_nothing_selected_the_controls_move_and_no_note_changes() {
        let (mut state, mut controls) = with_a3();
        state.selected.clear();
        assert!(apply_note_edit(&mut state, &mut controls, Action::Fret(5)));
        assert_eq!(controls.fret, 7);
        assert_eq!(at(&state, 0).fret, 2);
    }

    #[test]
    fn a_harmonic_takes_a_half_step_bend_from_the_bend_button() {
        assert_eq!(next_bend(StringTechnique::Harmonic), StringTechnique::Bend(1.0));
    }
    #[test]
    fn sustained_strings_and_hand_span_are_checked_before_mutation() {
        let mut state = guitar_state();
        let note = |id, string, fret, tick, len| GridNote {
            id,
            tick,
            len,
            expr: Expr::None,
            target: NoteTarget::Positioned(PlayPosition::Fret(FretAssignment {
                string,
                fret,
                technique: StringTechnique::Fretted,
            })),
        };
        state.notes.push(note(1, 1, 2, 0, 24));
        assert!(!valid(&state, &note(2, 1, 4, 12, 6)));
        assert!(!valid(&state, &note(2, 2, 10, 12, 6)));
        assert!(valid(&state, &note(2, 2, 4, 12, 6)));
        assert!(valid(&state, &note(2, 1, 10, 24, 6)));
    }
}

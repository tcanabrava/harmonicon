// SPDX-License-Identifier: MIT
//! Standard open-chord shapes with simultaneous, pitch-based feedback.
use super::*;
use harmonicon_audio::{AudioSettings, pitch_detect::PitchAlgorithm};
use harmonicon_ui::dialogs::{
    combobox::{ComboboxSelect, spawn_combobox},
    drawer::Drawer,
};

pub(super) struct Shape {
    pub name: &'static str,
    pub frets: &'static [Option<u8>],
    fingers: &'static [u8],
}
// String 1 first. None means do not strum; zero means open.
pub(super) const SHAPES: &[Shape] = &[
    Shape {
        name: "Em",
        frets: &[Some(0), Some(0), Some(0), Some(2), Some(2), Some(0)],
        fingers: &[0, 0, 0, 3, 2, 0],
    },
    Shape {
        name: "E",
        frets: &[Some(0), Some(0), Some(1), Some(2), Some(2), Some(0)],
        fingers: &[0, 0, 1, 3, 2, 0],
    },
    Shape {
        name: "Am",
        frets: &[Some(0), Some(1), Some(2), Some(2), Some(0), None],
        fingers: &[0, 1, 3, 2, 0, 0],
    },
    Shape {
        name: "A",
        frets: &[Some(0), Some(2), Some(2), Some(2), Some(0), None],
        fingers: &[0, 3, 2, 1, 0, 0],
    },
    Shape {
        name: "D",
        frets: &[Some(2), Some(3), Some(2), Some(0), None, None],
        fingers: &[2, 3, 1, 0, 0, 0],
    },
    Shape {
        name: "Dm",
        frets: &[Some(1), Some(3), Some(2), Some(0), None, None],
        fingers: &[1, 3, 2, 0, 0, 0],
    },
    Shape {
        name: "C",
        frets: &[Some(0), Some(1), Some(0), Some(2), Some(3), None],
        fingers: &[0, 1, 0, 2, 3, 0],
    },
    Shape {
        name: "G",
        frets: &[Some(3), Some(0), Some(0), Some(0), Some(2), Some(3)],
        fingers: &[3, 0, 0, 0, 1, 2],
    },
];
// Brazilian D4-G4-B4-D5 tuning, string 1 first. These voicings contain
// complete triads; finger numbers are suggestions, not microphone evidence.
pub(super) const CAVAQUINHO_SHAPES: &[Shape] = &[
    Shape { name: "G", frets: &[Some(0), Some(0), Some(0), Some(0)], fingers: &[0, 0, 0, 0] },
    Shape { name: "C", frets: &[Some(2), Some(1), Some(0), Some(2)], fingers: &[3, 1, 0, 2] },
    Shape { name: "D", frets: &[Some(4), Some(3), Some(2), Some(0)], fingers: &[3, 2, 1, 0] },
    Shape { name: "Em", frets: &[Some(2), Some(0), Some(0), Some(2)], fingers: &[2, 0, 0, 1] },
];

pub(super) fn shapes_for(setup: &Guitar) -> Option<&'static [Shape]> {
    use harmonicon_core::instrument::fretted_strings::FrettedKind;
    if setup.capo != 0 {
        return None;
    }
    match setup.kind {
        FrettedKind::Guitar
            if setup.strings == harmonicon_core::instrument::guitar::STANDARD_TUNING =>
        {
            Some(SHAPES)
        }
        FrettedKind::Cavaquinho
            if setup.strings == harmonicon_core::instrument::cavaquinho::BRAZILIAN_TUNING =>
        {
            Some(CAVAQUINHO_SHAPES)
        }
        _ => None,
    }
}

impl Shape {
    pub(super) fn positions(&self) -> Vec<FretAssignment> {
        self.frets
            .iter()
            .enumerate()
            .filter_map(|(i, &fret)| {
                fret.map(|fret| FretAssignment {
                    string: i as u8 + 1,
                    fret,
                    technique: StringTechnique::Fretted,
                })
            })
            .collect()
    }
    pub(super) fn pitches(&self, guitar: &Guitar) -> Vec<u8> {
        let mut notes: Vec<u8> =
            self.positions().iter().filter_map(|p| guitar.pitch_of(p)).collect();
        notes.sort_unstable();
        notes.dedup();
        notes
    }
}

pub(super) struct ChordPractice {
    shapes: &'static [Shape],
    pub index: usize,
    pub landed: u32,
    pub held: f32,
    pub awaiting_release: bool,
    present: Vec<u8>,
    extras: Vec<u8>,
}
impl Default for ChordPractice {
    fn default() -> Self {
        Self {
            shapes: SHAPES,
            index: 0,
            landed: 0,
            held: 0.0,
            awaiting_release: false,
            present: Vec::new(),
            extras: Vec::new(),
        }
    }
}
impl ChordPractice {
    pub fn new(shapes: &'static [Shape]) -> Self {
        Self { shapes, ..default() }
    }
    pub fn shape(&self) -> &'static Shape {
        &self.shapes[self.index]
    }
    pub fn step_by(&mut self, delta: isize) {
        let index = (self.index as isize + delta).rem_euclid(self.shapes.len() as isize) as usize;
        self.select(index);
    }
    fn select(&mut self, index: usize) {
        if index < self.shapes.len() && index != self.index {
            *self = Self { shapes: self.shapes, index, landed: self.landed, ..default() };
        }
    }
    /// Every expected absolute pitch must be in this frame. Do not collect
    /// individually plucked strings over time and call that a chord.
    pub fn listen(
        &mut self,
        expected: &[u8],
        heard: &[f32],
        dt: f32,
        tolerance: f32,
        hold: f32,
        polyphonic: bool,
    ) -> bool {
        self.present.clear();
        self.extras.clear();
        if !polyphonic || expected.is_empty() {
            self.held = 0.0;
            return false;
        }
        for &pitch in heard.iter().filter(|p| p.is_finite()) {
            let nearest = expected.iter().min_by(|&&a, &&b| {
                (pitch - f32::from(a)).abs().total_cmp(&(pitch - f32::from(b)).abs())
            });
            if let Some(&note) =
                nearest.filter(|&&note| (pitch - f32::from(note)).abs() * 100.0 <= tolerance)
            {
                if !self.present.contains(&note) {
                    self.present.push(note);
                }
            } else {
                let note = pitch.round() as u8;
                if !self.extras.contains(&note) {
                    self.extras.push(note);
                }
            }
        }
        if self.awaiting_release {
            if heard.is_empty() {
                self.awaiting_release = false;
            }
            self.held = 0.0;
            return false;
        }
        let complete = self.present.len() == expected.len() && self.extras.is_empty();
        self.held = if complete { self.held + dt } else { 0.0 };
        if self.held >= hold.max(0.01) {
            self.landed += 1;
            self.awaiting_release = true;
            return true;
        }
        false
    }
    pub fn status(&self, guitar: &Guitar, audio: &AudioSettings, loc: &Localization) -> String {
        if !audio.pitch_algorithm.is_polyphonic() {
            return loc.msg("trainer-chord-needs-poly").to_string();
        }
        if self.awaiting_release {
            return loc.msg("trainer-chord-release").to_string();
        }
        if !self.extras.is_empty() {
            return loc
                .msg_args("trainer-chord-extra", &[("notes", names(&self.extras))])
                .to_string();
        }
        let expected = self.shape().pitches(guitar);
        let missing: Vec<_> =
            expected.iter().filter(|n| !self.present.contains(n)).copied().collect();
        if missing.is_empty() {
            return loc.msg("trainer-chord-hold").to_string();
        }
        loc.msg_args(
            "trainer-chord-missing",
            &[
                ("heard", self.present.len().to_string()),
                ("total", expected.len().to_string()),
                ("notes", names(&missing)),
            ],
        )
        .to_string()
    }
}
fn names(notes: &[u8]) -> String {
    notes.iter().map(|&m| midi_to_note(i32::from(m))).collect::<Vec<_>>().join(", ")
}

#[derive(Component)]
pub(crate) struct Panel;
#[derive(Component)]
pub(crate) struct CircleLabel;

#[derive(Component)]
pub(crate) struct Mark {
    string: usize,
    above: bool,
}
#[derive(Component)]
pub(crate) struct BoardHelp;

pub(super) fn spawn(parent: &mut ChildSpawnerCommands, loc: &Localization) {
    parent
        .spawn((
            Panel,
            Drawer { open: false },
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(10.0),
                flex_shrink: 0.0,
                ..default()
            },
        ))
        .with_children(|p| {
            p.spawn((
                Text::new(loc.msg("trainer-chord-diagram-help").to_string()),
                TextFont { font_size: FontSize::Px(15.0), ..default() },
                TextColor(Color::WHITE),
            ));
            p.spawn((
                Text::new(loc.msg("trainer-chord-assessment-help").to_string()),
                TextFont { font_size: FontSize::Px(13.0), ..default() },
                TextColor(Color::srgb(0.75, 0.8, 0.85)),
            ));
            p.spawn(Node::default()).with_children(|row| {
                row.spawn_empty().apply_scene(button::small(
                    &loc.msg("trainer-chord-detector"),
                    |_: On<Activate>, mut audio: ResMut<AudioSettings>| {
                        audio.pitch_algorithm = PitchAlgorithm::Nmf
                    },
                ));
            });
        });
}

pub(super) fn spawn_diagram(parent: &mut ChildSpawnerCommands, setup: &Guitar) {
    parent
        .spawn((
            Panel,
            Drawer { open: false },
            Node {
                width: Val::Px(180.0),
                height: Val::Px(160.0),
                flex_shrink: 0.0,
                margin: UiRect::vertical(Val::Px(28.0)),
                ..default()
            },
        ))
        .with_children(|diagram| {
            let count = usize::from(setup.string_count());
            let names = setup.string_labels();
            for (string, name) in names.into_iter().enumerate() {
                let x = (count - 1 - string) as f32 * 100.0 / count.saturating_sub(1).max(1) as f32;
                diagram.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Percent(x),
                        height: Val::Percent(100.0),
                        width: Val::Px(1.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.7, 0.7, 0.75)),
                ));
                diagram.spawn((
                    Text::new(name),
                    TextFont { font_size: FontSize::Px(13.0), ..default() },
                    TextColor(Color::WHITE),
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Percent(x),
                        top: Val::Px(164.0),
                        width: Val::Px(24.0),
                        ..default()
                    },
                    UiTransform::from_translation(Val2::new(Val::Percent(-50.0), Val::Px(0.0))),
                ));
                for above in [true, false] {
                    diagram
                        .spawn((
                            Mark { string, above },
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Percent(x),
                                width: Val::Px(24.0),
                                height: Val::Px(24.0),
                                border_radius: BorderRadius::MAX,
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                ..default()
                            },
                            UiTransform::from_translation(Val2::all(Val::Percent(-50.0))),
                            BackgroundColor(if above {
                                Color::NONE
                            } else {
                                Color::srgb(0.98, 0.82, 0.35)
                            }),
                        ))
                        .with_children(|circle| {
                            circle.spawn((
                                CircleLabel,
                                Text::new(""),
                                TextFont { font_size: FontSize::Px(16.0), ..default() },
                                TextLayout::no_wrap(),
                                TextColor(if above { Color::WHITE } else { Color::BLACK }),
                            ));
                        });
                }
            }
            for fret in 0..=4 {
                diagram.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        top: Val::Percent(fret as f32 * 25.0),
                        width: Val::Percent(100.0),
                        height: Val::Px(if fret == 0 { 3.0 } else { 1.0 }),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.7, 0.7, 0.75)),
                ));
            }
        });
}

pub(crate) fn update(
    trainer: Res<NoteTrainer>,
    loc: Res<Localization>,
    mut panels: Query<&mut Drawer, With<Panel>>,
    mut marks: Query<(&Mark, &mut Node, &Children), Without<BoardHelp>>,
    mut labels: Query<&mut Text, With<CircleLabel>>,
    mut help: Query<&mut Text, (With<BoardHelp>, Without<Mark>, Without<CircleLabel>)>,
) {
    for mut panel in &mut panels {
        panel.open = trainer.chord.is_some();
    }
    for mut label in &mut help {
        label.0 = loc
            .msg(if trainer.chord.is_some() {
                "trainer-chord-board-help"
            } else {
                "trainer-board-help"
            })
            .to_string();
    }
    let Some(chord) = &trainer.chord else { return };
    let shape = chord.shape();
    for (mark, mut node, children) in &mut marks {
        let Ok(mut text) = labels.get_mut(children[0]) else { continue };
        let fret = shape.frets[mark.string];
        if mark.above {
            node.top = Val::Px(-18.0);
            node.display = Display::Flex;
            text.0 = match fret {
                None => "×".into(),
                Some(0) => "○".into(),
                _ => String::new(),
            };
        } else {
            node.display = if fret.is_some_and(|f| f > 0) { Display::Flex } else { Display::None };
            if let Some(fret) = fret {
                node.top = Val::Percent((f32::from(fret) - 0.5) * 25.0);
            }
            text.0 = shape.fingers[mark.string].to_string();
        }
    }
}

pub(super) fn controls(
    commands: &mut Commands,
    host: Entity,
    overlay: Entity,
    trainer: &NoteTrainer,
    loc: &Localization,
) {
    let Some(chord) = &trainer.chord else { return };
    let options: Vec<String> = chord.shapes.iter().map(|s| s.name.to_string()).collect();
    spawn_combobox(
        commands,
        host,
        overlay,
        &loc.msg("trainer-chord-choose"),
        &options,
        chord.shape().name,
        |ev: On<ComboboxSelect>, mut trainer: ResMut<NoteTrainer>| {
            if let Some(chord) = &mut trainer.chord {
                chord.select(ev.index);
            }
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn individual_strings_cannot_accumulate_into_a_chord() {
        let guitar = Guitar::standard(GuitarBody::Electric);
        let mut practice = ChordPractice::default();
        let expected = practice.shape().pitches(&guitar);
        for &note in &expected {
            assert!(!practice.listen(&expected, &[f32::from(note)], 1.0, 20.0, 0.6, true));
        }
        let heard: Vec<f32> = expected.iter().map(|&n| f32::from(n)).collect();
        assert!(
            !practice.listen(&expected, &heard, 1.0, 20.0, 0.6, false),
            "monophonic mode cannot assess a chord"
        );
        assert!(practice.listen(&expected, &heard, 0.7, 20.0, 0.6, true));
        assert!(!practice.listen(&expected, &heard, 1.0, 20.0, 0.6, true));
        assert_eq!(practice.landed, 1);
        practice.listen(&expected, &[], 0.1, 20.0, 0.6, true);
        let mut wrong = heard.clone();
        wrong.push(61.0);
        assert!(
            !practice.listen(&expected, &wrong, 1.0, 20.0, 0.6, true),
            "an extra pitch is not a clean chord"
        );
        assert!(practice.listen(&expected, &heard, 0.7, 20.0, 0.6, true));
        assert_eq!(practice.landed, 2);
    }
    #[test]
    fn standard_shapes_match_their_chord_names_and_keep_muted_strings_out() {
        let guitar = Guitar::standard(GuitarBody::Electric);
        let e = SHAPES[0].pitches(&guitar);
        assert!(e.iter().all(|n| [4, 7, 11].contains(&(n % 12))), "Em contains E, G, B");
        assert_eq!(SHAPES[4].positions().len(), 4, "D omits both bass strings");
        let classes = [
            [4, 7, 11],
            [4, 8, 11],
            [9, 0, 4],
            [9, 1, 4],
            [2, 6, 9],
            [2, 5, 9],
            [0, 4, 7],
            [7, 11, 2],
        ];
        for (shape, mut expected) in SHAPES.iter().zip(classes) {
            let mut actual: Vec<_> = shape.pitches(&guitar).into_iter().map(|n| n % 12).collect();
            actual.sort_unstable();
            actual.dedup();
            expected.sort_unstable();
            assert_eq!(actual, expected, "{} must sound its named major/minor triad", shape.name);
            assert!(shape.positions().iter().all(|p| guitar.pitch_of(p).is_some()));
            assert!(
                shape
                    .frets
                    .iter()
                    .zip(shape.fingers.iter().copied())
                    .all(|(f, finger)| f.is_none_or(|f| f == 0 || (1..=4).contains(&finger)))
            );
        }
    }
}

#[cfg(test)]
mod cavaquinho_tests {
    use super::*;
    #[test]
    fn four_string_shapes_sound_complete_triads_and_keep_their_own_library() {
        let setup = Guitar::cavaquinho();
        let mut practice = ChordPractice::new(shapes_for(&setup).unwrap());
        for (shape, mut expected) in CAVAQUINHO_SHAPES.iter().zip([
            vec![2, 7, 11],
            vec![0, 4, 7],
            vec![2, 6, 9],
            vec![4, 7, 11],
        ]) {
            assert_eq!(shape.frets.len(), 4);
            assert_eq!(shape.fingers.len(), 4);
            let mut actual: Vec<_> =
                shape.pitches(&setup).into_iter().map(|pitch| pitch % 12).collect();
            actual.sort_unstable();
            actual.dedup();
            expected.sort_unstable();
            assert_eq!(actual, expected, "{}", shape.name);
            assert_eq!(practice.shape().name, shape.name);
            practice.step_by(1);
        }
        assert_eq!(practice.shape().name, "G");
        let mut alternate = setup;
        alternate.strings = harmonicon_core::instrument::cavaquinho::TUNINGS[1].strings.to_vec();
        assert!(shapes_for(&alternate).is_none(), "a shape is tied to its actual tuning");
    }
}

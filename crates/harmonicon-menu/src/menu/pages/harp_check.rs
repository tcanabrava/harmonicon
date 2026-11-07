// SPDX-License-Identifier: MIT

//! "Which harmonica are you holding?" — or which tuning is the guitar in,
//! which foot joint has the flute — shown after picking a song, before it
//! loads.
//!
//! A chart names the instrument it was written for, and until this page
//! existed that was a hard requirement: own a G and a C-harp chart was
//! simply unplayable. Here the player can say what they actually have. A
//! harmonica also picks what the swap should preserve
//! (`harmonicon_core::harp_remap`); a guitar or a flute always keeps the
//! pitches and moves the positions (`instrument::substitute`).
//!
//! The page builds all three sets of controls and shows the chart's: the
//! chart may still be decoding when it is built, so which instrument it is
//! is only known on the frame it arrives (`refresh_harp_cost`).
//!
//! **The cost readout is the point, not decoration.** Under
//! [`HarpMapping::Transpose`] a different harp can turn a plain melody into
//! an overblow study, or put notes out of reach entirely. Saying so *before*
//! the song starts is what makes the choice informed rather than a surprise
//! three bars in.
//!
//! Deliberately only on the song-list route. A lesson prescribes its own
//! harmonica as part of the teaching, and the guided tour drives itself —
//! interposing a question in either would be wrong.

use bevy::prelude::*;
use bevy::ui_widgets::Activate;

use harmonicon_app::app::{AppState, EffectiveHarmonica, SelectedSong};
use harmonicon_core::chart::HarpChart;
use harmonicon_core::harmonica::{Harmonica, detected_harp_key};
use harmonicon_core::harp_remap::{HarpMapping, RemapCost, remap_event};
use harmonicon_core::instrument::Instrument;
use harmonicon_core::instrument::flute::{Flute, FootJoint};
use harmonicon_core::instrument::fretted_strings::FrettedKind;
use harmonicon_core::instrument::guitar::Guitar;
#[cfg(test)]
use harmonicon_core::instrument::guitar::TUNINGS;
#[cfg(test)]
use harmonicon_core::instrument::guitar::tuning_of;
use harmonicon_core::instrument::substitute::substitute_chart;
use harmonicon_core::pitch_map::{HARP_KEYS, HarpKind, harp_for_key};
use harmonicon_platform::localization::{Localization, LocalizationExt};
use harmonicon_platform::settings::MyInstruments;
use harmonicon_platform::theme::LoadedTheme;
use harmonicon_song::song::{SongManifest, TrackChart};
use harmonicon_ui::dialogs::combobox;

use crate::menu::routing::MenuPage;
use crate::menu::scene::{MenuRoot, spawn_back_button, spawn_button, spawn_menu_root};

/// What the player has said they're holding. Seeded from the chart on entry,
/// so the page opens on "what the chart asked for" and any change is a
/// deliberate one.
#[derive(Resource, Clone, Debug)]
pub(crate) struct HarpChoice {
    pub key: String,
    pub kind: HarpKind,
    pub mapping: HarpMapping,
    /// The guitar or flute the player holds, for a chart written for one:
    /// the chart's own until a tuning or foot joint is picked.
    pub played: Option<Instrument>,
    /// Whether the fields above have been pointed at the chart yet. The
    /// chart may still be decoding when this page is built, so seeding can't
    /// simply happen on entry; it happens on whichever frame the manifest
    /// first resolves. A field rather than a `Local` because a `Local`
    /// belongs to the system forever and would not reset between songs.
    pub seeded: bool,
}

impl Default for HarpChoice {
    fn default() -> Self {
        Self {
            key: HARP_KEYS[0].to_string(),
            kind: HarpKind::Diatonic,
            mapping: HarpMapping::default(),
            played: None,
            seeded: false,
        }
    }
}

impl HarpChoice {
    fn harp(&self) -> Harmonica {
        harp_for_key(&self.key, self.kind)
    }

    /// Whether this is still the harmonica the chart asked for. When it is,
    /// gameplay is left on its untouched default path rather than routed
    /// through the remapper for no reason.
    fn matches_chart(&self, chart: &HarpChart) -> bool {
        let Some(harp) = &chart.harmonica else {
            return false;
        };
        let same_kind = matches!(
            (harp, self.kind),
            (Harmonica::Diatonic { .. }, HarpKind::Diatonic)
                | (Harmonica::Chromatic { .. }, HarpKind::Chromatic)
        );
        same_kind && detected_harp_key(harp).as_deref() == Some(self.key.as_str())
    }
}

/// The line reporting what the current choice will demand.
#[derive(Component)]
pub(crate) struct HarpCostLabel;

/// The line naming the harmonica the chart was written for.
#[derive(Component)]
pub(crate) struct ChartHarpLabel;

/// The page's opening line, worded for the chart's instrument.
#[derive(Component)]
pub(crate) struct IntroLabel;

#[derive(Component)]
pub(crate) struct InstrumentControlsKind(Option<harmonicon_core::instrument::InstrumentKind>);

/// Which instrument's controls a container holds; only the chart's shows.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ControlsFor {
    Harmonica,
    Fretted,
    Flute,
    Clarinet,
    Bowed,
    /// Nothing to choose yet: a part's keyboard is the one it is played on.
    Piano,
}

fn controls_for(chart: &HarpChart) -> ControlsFor {
    match chart.instrument() {
        Some(Instrument::Fretted(_)) => ControlsFor::Fretted,
        Some(Instrument::Flute(_)) => ControlsFor::Flute,
        Some(Instrument::Clarinet(_)) => ControlsFor::Clarinet,
        Some(Instrument::Bowed(_)) => ControlsFor::Bowed,
        Some(Instrument::Piano(_)) => ControlsFor::Piano,
        _ => ControlsFor::Harmonica,
    }
}

/// A tuning's name: `guitar-tuning-<id>`, or "as written" for a chart's
/// tuning none of the named ones match.
fn tuning_label(id: Option<&str>, loc: &Localization) -> String {
    match id {
        Some(id) => String::from(loc.msg(&format!("guitar-tuning-{id}"))),
        None => String::from(loc.msg("guitar-tuning-charted")),
    }
}

fn foot_label(foot: FootJoint, loc: &Localization) -> String {
    String::from(loc.msg(match foot {
        FootJoint::C => "flute-foot-c",
        FootJoint::B => "flute-foot-b",
    }))
}

/// The guitar or flute the player holds for a chart written for `own`: the
/// chart's own, with "My instruments"' tuning or foot joint put on it when
/// the player has named one. `None` for a harmonica, which this page asks
/// about separately.
pub(crate) fn played_for(own: Option<Instrument>, mine: &MyInstruments) -> Option<Instrument> {
    match own? {
        Instrument::Fretted(guitar) => {
            let saved_tuning = mine.fretted_tuning(guitar.kind);
            let saved_capo = mine.fretted_capo(guitar.kind);
            let tuning =
                saved_tuning.and_then(|id| guitar.tuning_presets().iter().find(|t| t.id == id));
            let strings = tuning.map_or_else(|| guitar.strings.clone(), |t| t.strings.to_vec());
            let capo = saved_capo.map_or(guitar.capo, |c| c.min(MAX_CAPO));
            Some(Guitar { strings, capo, ..guitar }.into())
        }
        Instrument::Flute(flute) => Some(Flute::new(mine.flute_foot.unwrap_or(flute.foot)).into()),
        Instrument::Clarinet(setup) => Some(
            harmonicon_core::instrument::clarinet::Clarinet {
                key: mine.clarinet_key.unwrap_or(setup.key),
            }
            .into(),
        ),
        Instrument::Bowed(strings) => Some(strings.into()),
        Instrument::Piano(piano) => Some(piano.into()),
        Instrument::Harmonica(_) => None,
    }
}

/// The highest fret a capo is offered on.
pub(crate) const MAX_CAPO: u8 = 9;

/// A capo position's name: "No capo" or "Fret 3".
pub(crate) fn capo_label(capo: u8, loc: &Localization) -> String {
    if capo == 0 {
        String::from(loc.msg("guitar-capo-none"))
    } else {
        String::from(loc.msg_args("guitar-capo-fret", &[("fret", capo.to_string())]))
    }
}

/// `first` (following each chart), then no capo, then each fret up to
/// [`MAX_CAPO`].
pub(crate) fn capo_options(first: String, loc: &Localization) -> Vec<String> {
    std::iter::once(first).chain((0..=MAX_CAPO).map(|c| capo_label(c, loc))).collect()
}

/// The capo an option of [`capo_options`] names; `None` for the first.
pub(crate) fn capo_from_index(index: usize) -> Option<u8> {
    index.checked_sub(1).and_then(|i| u8::try_from(i).ok()).filter(|c| *c <= MAX_CAPO)
}

/// Where `capo` sits in [`capo_options`].
pub(crate) fn capo_index(capo: Option<u8>) -> usize {
    capo.filter(|c| *c <= MAX_CAPO).map_or(0, |c| usize::from(c) + 1)
}

/// `(moved, unreachable)`: how many of `chart`'s notes `played` puts
/// somewhere other than the chart did, and how many it can't sound.
pub(crate) fn substitution_cost(chart: &HarpChart, played: &Instrument) -> (usize, usize) {
    let Some(own) = chart.instrument() else { return (0, 0) };
    let charted = substitute_chart(chart, &own);
    let placed = substitute_chart(chart, played);
    let pairs = || charted.iter().flatten().zip(placed.iter().flatten());
    let unreachable = pairs().filter(|(_, p)| p.is_none()).count();
    let moved = pairs().filter(|(c, p)| p.is_some() && c != p).count();
    (moved, unreachable)
}

/// What the player's guitar or flute changes for `chart`: how many notes
/// move to another place, and how many it can't sound. Empty when nothing
/// does.
pub(crate) fn substitution_message(
    chart: &HarpChart,
    played: &Instrument,
    loc: &Localization,
) -> String {
    if chart.harmonica.is_some() {
        return String::new();
    }
    let (moved, unreachable) = substitution_cost(chart, played);
    if unreachable == 0 && moved == 0 {
        return String::from(loc.msg("instrument-check-cost-clean"));
    }
    let mut parts = Vec::new();
    if moved > 0 {
        let key = match played {
            Instrument::Flute(_) | Instrument::Clarinet(_) => "instrument-check-cost-fingerings",
            Instrument::Fretted(setup) if setup.kind == FrettedKind::Cavaquinho => {
                "instrument-check-cost-cavaquinho-frets"
            }
            Instrument::Fretted(setup) if setup.kind == FrettedKind::Bass => {
                "instrument-check-cost-bass-frets"
            }
            _ => "instrument-check-cost-frets",
        };
        parts.push(String::from(loc.msg_args(key, &[("count", moved.to_string())])));
    }
    if unreachable > 0 {
        parts.push(String::from(
            loc.msg_args(
                "instrument-check-cost-unreachable",
                &[("count", unreachable.to_string())],
            ),
        ));
    }
    parts.join("  ·  ")
}

fn kind_label(kind: HarpKind, loc: &Localization) -> String {
    String::from(match kind {
        HarpKind::Diatonic => loc.msg("harp-kind-diatonic"),
        HarpKind::Chromatic => loc.msg("harp-kind-chromatic"),
    })
}

fn kind_labels(loc: &Localization) -> Vec<String> {
    vec![kind_label(HarpKind::Diatonic, loc), kind_label(HarpKind::Chromatic, loc)]
}

/// Resolved against the same localized labels the list was built from, for
/// the same reason the mapping combobox is: matching an English literal
/// would stop working the moment the player isn't running in English.
fn kind_from_label(label: &str, loc: &Localization) -> HarpKind {
    if label == kind_label(HarpKind::Chromatic, loc) {
        HarpKind::Chromatic
    } else {
        HarpKind::Diatonic
    }
}

/// "C diatonic" — the harmonica a chart asks for, named rather than merely
/// alluded to. Without this the page said a song "was written for one
/// harmonica" and never said which, which is the one fact a player needs
/// before they can tell whether theirs differs.
pub(crate) fn harp_name(harp: &Harmonica, loc: &Localization) -> String {
    let kind = match harp {
        Harmonica::Chromatic { .. } => HarpKind::Chromatic,
        Harmonica::Diatonic { .. } => HarpKind::Diatonic,
    };
    let key = detected_harp_key(harp).unwrap_or_else(|| "?".to_string());
    String::from(
        loc.msg_args("harp-check-chart-harp", &[("key", key), ("kind", kind_label(kind, loc))]),
    )
}

fn mapping_labels(loc: &Localization) -> Vec<String> {
    vec![
        String::from(loc.msg("harp-check-same-holes")),
        String::from(loc.msg("harp-check-transpose")),
    ]
}

/// Every event of `chart`, resolved onto `choice`'s harp.
pub(crate) fn cost_of(chart: &HarpChart, choice: &HarpChoice) -> RemapCost {
    let target = choice.harp();
    let mut cost = RemapCost::default();
    let Some(source) = chart.harmonica.as_ref() else {
        return cost;
    };
    for item in &chart.track {
        for event in &item.events {
            let Some((hole, action)) = event.hole_action() else {
                continue;
            };
            cost.add(&remap_event(
                hole,
                action,
                event.note.as_deref(),
                event.modifiers.as_deref().unwrap_or(&[]),
                source,
                &target,
                choice.mapping,
            ));
        }
    }
    cost
}

/// A `HarpChoice` pointing at the chart's own harmonica — the baseline
/// every other choice is priced against.
pub(crate) fn baseline_choice(chart: &HarpChart) -> HarpChoice {
    let mut c = HarpChoice::default();
    seed_choice(&mut c, chart, &MyInstruments::default());
    c
}

/// The cost line's text, or the "nothing to report" message.
///
/// Reports what the *swap* demands, not what the music does. A blues lick
/// is full of bends on any harmonica, and saying "6 notes need a bend" when
/// nothing has been changed reads as a warning about a choice the player
/// hasn't made — which is exactly what the first version did. Only the
/// techniques a substitution *adds* over `baseline` are worth naming.
///
/// Unplayable notes are reported absolutely: on the chart's own harp there
/// are none, so any at all are new.
pub(crate) fn cost_message(cost: &RemapCost, baseline: &RemapCost, loc: &Localization) -> String {
    if cost.total == 0 {
        return String::new();
    }
    let added_bends = cost.bends.saturating_sub(baseline.bends);
    let added_overblows = cost.overblows.saturating_sub(baseline.overblows);
    if cost.is_complete() && added_bends == 0 && added_overblows == 0 {
        return String::from(loc.msg("harp-check-cost-clean"));
    }
    let mut parts: Vec<String> = Vec::new();
    if added_bends > 0 {
        parts.push(String::from(
            loc.msg_args("harp-check-cost-bends", &[("count", added_bends.to_string())]),
        ));
    }
    if added_overblows > 0 {
        parts.push(String::from(
            loc.msg_args("harp-check-cost-overblows", &[("count", added_overblows.to_string())]),
        ));
    }
    if cost.unplayable > 0 {
        parts.push(String::from(
            loc.msg_args("harp-check-cost-unreachable", &[("count", cost.unplayable.to_string())]),
        ));
    }
    parts.join("  ·  ")
}

pub(crate) fn setup_harp_check(
    mut commands: Commands,
    theme: Res<LoadedTheme>,
    loc: Res<Localization>,
    selected: Option<Res<SelectedSong>>,
    manifests: Res<Assets<SongManifest>>,
    mut choice: ResMut<HarpChoice>,
    mine: Res<MyInstruments>,
) {
    // Seed from the chart when it's already resident (the common case: the
    // song list holds a strong handle, so it is usually decoded by now).
    // When it isn't, `refresh_harp_cost` seeds on the frame it arrives.
    if let Some(chart) = selected.as_ref().and_then(|s| manifests.get(&s.0)).map(|m| &m.chart) {
        seed_choice(&mut choice, chart, &mine);
    }

    let chart = selected.as_ref().and_then(|s| manifests.get(&s.0)).map(|m| &m.chart);
    let shown = chart.map_or(ControlsFor::Harmonica, controls_for);
    let is_bass = chart.and_then(HarpChart::instrument).is_some_and(|instrument| {
        instrument.kind() == harmonicon_core::instrument::InstrumentKind::Bass
    });
    let title = if shown == ControlsFor::Bowed {
        format!(
            "song-instrument-{}",
            chart.and_then(HarpChart::instrument).expect("bowed chart").kind().slug()
        )
    } else {
        match shown {
            ControlsFor::Harmonica => "harp-check-title",
            ControlsFor::Fretted
                if chart.and_then(HarpChart::instrument).is_some_and(|instrument| {
                    instrument.kind() == harmonicon_core::instrument::InstrumentKind::Cavaquinho
                }) =>
            {
                "instrument-check-title-cavaquinho"
            }
            ControlsFor::Fretted if is_bass => "instrument-check-title-bass",
            ControlsFor::Fretted => "instrument-check-title-guitar",
            ControlsFor::Flute => "instrument-check-title-flute",
            ControlsFor::Clarinet => "instrument-clarinet",
            ControlsFor::Bowed => unreachable!("bowed title is resolved above"),
            ControlsFor::Piano => "instrument-check-title-piano",
        }
        .to_owned()
    };
    let (root, header, page_root) =
        spawn_menu_root(&mut commands, &loc.msg(&title), None, &theme, "HarpCheck");
    commands.entity(page_root).insert(InstrumentControlsKind(
        chart.and_then(HarpChart::instrument).map(|instrument| instrument.kind()),
    ));

    let intro = commands
        .spawn_empty()
        .apply_scene(bsn! {
            Text({String::from(loc.msg("harp-check-intro"))})
            TextFont { font_size: {FontSize::Px(17.0)} }
            TextColor({Color::srgb(0.82, 0.82, 0.88)})
            Node { max_width: {Val::Px(560.0)} }
        })
        .insert(IntroLabel)
        .id();
    commands.entity(root).add_child(intro);

    let written_for = commands
        .spawn_empty()
        .apply_scene(bsn! {
            Text("")
            TextFont { font_size: {FontSize::Px(18.0)} }
            TextColor({Color::srgb(0.95, 0.95, 1.0)})
        })
        .insert(ChartHarpLabel)
        .id();
    commands.entity(root).add_child(written_for);

    // One container per instrument; `refresh_harp_cost` shows the chart's.
    let container = |commands: &mut Commands, which: ControlsFor| {
        let display = if which == shown { Display::Flex } else { Display::None };
        let entity = commands
            .spawn((
                which,
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(12.0),
                    display,
                    ..default()
                },
            ))
            .id();
        commands.entity(root).add_child(entity);
        entity
    };
    let harp_box = container(&mut commands, ControlsFor::Harmonica);
    let guitar_box = container(&mut commands, ControlsFor::Fretted);
    let flute_box = container(&mut commands, ControlsFor::Flute);

    // A guitar's tuning: the named ones, plus the chart's own when it is
    // none of them. Picking one places the chart's notes onto it.
    let setup = match chart.and_then(HarpChart::instrument) {
        Some(Instrument::Fretted(setup)) => setup,
        _ => FrettedKind::Guitar.default_setup(),
    };
    let presets = setup.tuning_presets();
    let mut tunings: Vec<String> = presets.iter().map(|t| tuning_label(Some(t.id), &loc)).collect();
    tunings.insert(0, tuning_label(None, &loc));
    let current_tuning = match &choice.played {
        Some(Instrument::Fretted(g)) => tuning_label(g.named_tuning().map(|t| t.id), &loc),
        _ => tuning_label(None, &loc),
    };
    combobox::spawn_combobox(
        &mut commands,
        guitar_box,
        page_root,
        &loc.msg("instrument-check-tuning"),
        &tunings,
        &current_tuning,
        |ev: On<combobox::ComboboxSelect>,
         mut choice: ResMut<HarpChoice>,
         mut mine: ResMut<MyInstruments>,
         selected: Option<Res<SelectedSong>>,
         manifests: Res<Assets<SongManifest>>| {
            let chart = selected.as_ref().and_then(|s| manifests.get(&s.0)).map(|m| &m.chart);
            let own = chart.and_then(HarpChart::instrument);
            if !matches!(own, Some(Instrument::Fretted(_))) {
                return;
            }
            // Remembered: the next guitar chart opens on this tuning too.
            // "As written" goes back to following each chart.
            let Some(Instrument::Fretted(setup)) = &own else { return };
            let picked = ev
                .index
                .checked_sub(1)
                .and_then(|index| setup.tuning_presets().get(index))
                .map(|preset| preset.id.to_owned());
            mine.set_fretted_tuning(setup.kind, picked);
            choice.played = played_for(own, &mine);
        },
    );

    // A guitar's capo: as written first, then none, then each fret.
    let capos = capo_options(String::from(loc.msg("guitar-tuning-charted")), &loc);
    combobox::spawn_combobox(
        &mut commands,
        guitar_box,
        page_root,
        &loc.msg("instrument-check-capo"),
        &capos,
        &capos[capo_index(mine.fretted_capo(setup.kind))],
        |ev: On<combobox::ComboboxSelect>,
         mut choice: ResMut<HarpChoice>,
         mut mine: ResMut<MyInstruments>,
         selected: Option<Res<SelectedSong>>,
         manifests: Res<Assets<SongManifest>>| {
            let chart = selected.as_ref().and_then(|s| manifests.get(&s.0)).map(|m| &m.chart);
            let own = chart.and_then(HarpChart::instrument);
            if !matches!(own, Some(Instrument::Fretted(_))) {
                return;
            }
            // Remembered like the tuning; "as written" follows each chart.
            if let Some(Instrument::Fretted(setup)) = &own {
                mine.set_fretted_capo(setup.kind, capo_from_index(ev.index));
            }
            choice.played = played_for(own, &mine);
        },
    );

    let clarinet_box = container(&mut commands, ControlsFor::Clarinet);
    let clarinets = vec![
        loc.msg("clarinet-as-written").to_string(),
        loc.msg("clarinet-bb").to_string(),
        loc.msg("clarinet-a").to_string(),
    ];
    let index = match mine.clarinet_key {
        None => 0,
        Some(harmonicon_core::instrument::clarinet::ClarinetKey::Bb) => 1,
        Some(_) => 2,
    };
    combobox::spawn_combobox(
        &mut commands,
        clarinet_box,
        page_root,
        &loc.msg("clarinet-setup"),
        &clarinets,
        &clarinets[index],
        |ev: On<combobox::ComboboxSelect>,
         mut choice: ResMut<HarpChoice>,
         mut mine: ResMut<MyInstruments>,
         selected: Option<Res<SelectedSong>>,
         manifests: Res<Assets<SongManifest>>| {
            mine.clarinet_key = match ev.index {
                1 => Some(harmonicon_core::instrument::clarinet::ClarinetKey::Bb),
                2 => Some(harmonicon_core::instrument::clarinet::ClarinetKey::A),
                _ => None,
            };
            let own = selected
                .as_ref()
                .and_then(|s| manifests.get(&s.0))
                .and_then(|m| m.chart.instrument());
            if matches!(own, Some(Instrument::Clarinet(_))) {
                choice.played = played_for(own, &mine);
            }
        },
    );

    // A flute's foot joint.
    let feet = vec![foot_label(FootJoint::C, &loc), foot_label(FootJoint::B, &loc)];
    let current_foot = match &choice.played {
        Some(Instrument::Flute(f)) => foot_label(f.foot, &loc),
        _ => foot_label(FootJoint::C, &loc),
    };
    combobox::spawn_combobox(
        &mut commands,
        flute_box,
        page_root,
        &loc.msg("instrument-check-foot"),
        &feet,
        &current_foot,
        |ev: On<combobox::ComboboxSelect>,
         mut choice: ResMut<HarpChoice>,
         mut mine: ResMut<MyInstruments>,
         loc: Res<Localization>| {
            let foot = if ev.value == foot_label(FootJoint::B, &loc) {
                FootJoint::B
            } else {
                FootJoint::C
            };
            // Remembered: a flute's foot joint doesn't change between songs.
            mine.flute_foot = Some(foot);
            choice.played = Some(Flute::new(foot).into());
        },
    );

    combobox::spawn_combobox(
        &mut commands,
        harp_box,
        page_root,
        &loc.msg("harp-check-key"),
        &HARP_KEYS.iter().map(|k| k.to_string()).collect::<Vec<_>>(),
        &choice.key,
        |ev: On<combobox::ComboboxSelect>, mut choice: ResMut<HarpChoice>| {
            choice.key = ev.value.clone();
        },
    );

    combobox::spawn_combobox(
        &mut commands,
        harp_box,
        page_root,
        &loc.msg("harp-check-type"),
        &kind_labels(&loc),
        &kind_label(choice.kind, &loc),
        |ev: On<combobox::ComboboxSelect>,
         mut choice: ResMut<HarpChoice>,
         loc: Res<Localization>| {
            choice.kind = kind_from_label(&ev.value, &loc);
        },
    );

    let mapping_options = mapping_labels(&loc);
    let current_mapping = &mapping_options[usize::from(choice.mapping == HarpMapping::Transpose)];
    combobox::spawn_combobox(
        &mut commands,
        harp_box,
        page_root,
        &loc.msg("harp-check-mapping"),
        &mapping_options,
        current_mapping,
        |ev: On<combobox::ComboboxSelect>,
         mut choice: ResMut<HarpChoice>,
         loc: Res<Localization>| {
            // Compared against the same localized labels the list was built
            // from, so this keeps working in any locale.
            let options = mapping_labels(&loc);
            choice.mapping = if ev.value == options[1] {
                HarpMapping::Transpose
            } else {
                HarpMapping::SameHoles
            };
        },
    );

    // Filled in later, not here: the manifest is usually resident by now but
    // isn't guaranteed to be, and the track list can only be read off it.
    // `spawn_track_picker` populates this the frame it arrives.
    let track_slot = commands.spawn((Node::default(), TrackPickerSlot)).id();
    commands.entity(root).add_child(track_slot);

    let cost = commands
        .spawn_empty()
        .apply_scene(bsn! {
            Text("")
            TextFont { font_size: {FontSize::Px(16.0)} }
            TextColor({Color::srgb(0.95, 0.8, 0.45)})
            Node { max_width: {Val::Px(560.0)} }
        })
        .insert(HarpCostLabel)
        .id();
    commands.entity(root).add_child(cost);

    spawn_button(
        &mut commands,
        root,
        &loc.msg("harp-check-play"),
        |_: On<Activate>,
         choice: Res<HarpChoice>,
         selected: Option<Res<SelectedSong>>,
         manifests: Res<Assets<SongManifest>>,
         mut effective: ResMut<EffectiveHarmonica>,
         mut state: ResMut<NextState<AppState>>| {
            let chart = selected.as_ref().and_then(|s| manifests.get(&s.0)).map(|m| &m.chart);
            // Leave the default alone when nothing was actually changed, so
            // an untouched song stays on gameplay's original path.
            effective.harp = match chart {
                // Nothing to substitute on a chart for another instrument.
                Some(chart) if chart.harmonica.is_none() || choice.matches_chart(chart) => None,
                _ => Some(choice.harp()),
            };
            effective.mapping = choice.mapping;
            // A guitar's tuning or a flute's foot joint, only when it
            // differs from the chart's own.
            effective.instrument = choice
                .played
                .clone()
                .filter(|played| chart.and_then(HarpChart::instrument).as_ref() != Some(played));
            state.set(AppState::SongLoading);
        },
    );

    spawn_back_button(
        &mut commands,
        header,
        &loc.msg("back"),
        |_: On<Activate>, mut page: ResMut<NextState<MenuPage>>| page.set(MenuPage::ArtistList),
    );
}

/// Points `choice` at whatever the chart asks for, or for a guitar or a
/// flute at the one in "My instruments".
fn seed_choice(choice: &mut HarpChoice, chart: &HarpChart, mine: &MyInstruments) {
    if let Some(harp) = &chart.harmonica {
        if let Some(key) = detected_harp_key(harp) {
            choice.key = key;
        }
        choice.kind = match harp {
            Harmonica::Chromatic { .. } => HarpKind::Chromatic,
            Harmonica::Diatonic { .. } => HarpKind::Diatonic,
        };
    }
    choice.mapping = HarpMapping::default();
    choice.played = played_for(chart.instrument(), mine);
    choice.seeded = true;
}

/// Recomputes the cost line whenever the choice changes — or when the chart
/// finally decodes, which may be after this page was built.
pub(crate) fn refresh_harp_cost(
    mut commands: Commands,
    controls: Query<(Entity, &InstrumentControlsKind)>,
    mut choice: ResMut<HarpChoice>,
    mine: Res<MyInstruments>,
    loc: Res<Localization>,
    selected: Option<Res<SelectedSong>>,
    manifests: Res<Assets<SongManifest>>,
    mut labels: Query<&mut Text, With<HarpCostLabel>>,
    mut harp_labels: Query<&mut Text, (With<ChartHarpLabel>, Without<HarpCostLabel>)>,
    mut intros: Query<
        &mut Text,
        (With<IntroLabel>, Without<HarpCostLabel>, Without<ChartHarpLabel>),
    >,
    mut boxes: Query<(&ControlsFor, &mut Node)>,
) {
    let Some(chart) = selected.as_ref().and_then(|s| manifests.get(&s.0)).map(|m| &m.chart) else {
        return;
    };
    // A chart may decode after this page first mounts. Recreate the controls
    // with its identity before offering tuning choices from the wrong family.
    let kind = chart.instrument().map(|instrument| instrument.kind());
    if let Some((root, _)) = controls.iter().find(|(_, built)| built.0 != kind) {
        commands.entity(root).despawn();
        commands.run_system_cached(setup_harp_check);
        return;
    }
    if !choice.seeded {
        seed_choice(&mut choice, chart, &mine);
    } else if !choice.is_changed() && !loc.is_changed() {
        return;
    }
    let shown = controls_for(chart);
    for (which, mut node) in &mut boxes {
        let want = if *which == shown { Display::Flex } else { Display::None };
        if node.display != want {
            node.display = want;
        }
    }
    let is_bass = chart.instrument().is_some_and(|instrument| {
        instrument.kind() == harmonicon_core::instrument::InstrumentKind::Bass
    });
    let intro = String::from(loc.msg(match shown {
        ControlsFor::Harmonica => "harp-check-intro",
        ControlsFor::Fretted
            if chart.instrument().is_some_and(|instrument| {
                instrument.kind() == harmonicon_core::instrument::InstrumentKind::Cavaquinho
            }) =>
        {
            "instrument-check-intro-cavaquinho"
        }
        ControlsFor::Fretted if is_bass => "instrument-check-intro-bass",
        ControlsFor::Fretted => "instrument-check-intro-guitar",
        ControlsFor::Flute => "instrument-check-intro-flute",
        ControlsFor::Clarinet => "clarinet-play-help",
        ControlsFor::Bowed => "bowed-play-help",
        ControlsFor::Piano => "instrument-check-intro-piano",
    }));
    for mut label in &mut intros {
        if label.0 != intro {
            label.0.clone_from(&intro);
        }
    }
    let written_for = match chart.instrument() {
        Some(Instrument::Harmonica(harp)) => harp_name(&harp, &loc),
        Some(Instrument::Fretted(guitar)) => {
            let tuning = tuning_label(guitar.named_tuning().map(|t| t.id), &loc);
            String::from(if guitar.capo == 0 {
                loc.msg_args(
                    &format!("instrument-check-chart-{}", guitar.kind.instrument_kind().slug()),
                    &[("tuning", tuning)],
                )
            } else {
                loc.msg_args(
                    &format!(
                        "instrument-check-chart-{}-capo",
                        guitar.kind.instrument_kind().slug()
                    ),
                    &[("tuning", tuning), ("fret", guitar.capo.to_string())],
                )
            })
        }
        Some(Instrument::Flute(flute)) => String::from(
            loc.msg_args("instrument-check-chart-flute", &[("foot", foot_label(flute.foot, &loc))]),
        ),
        Some(Instrument::Clarinet(setup)) => loc
            .msg(if setup.key == harmonicon_core::instrument::clarinet::ClarinetKey::Bb {
                "clarinet-bb"
            } else {
                "clarinet-a"
            })
            .to_string(),
        Some(Instrument::Bowed(strings)) => strings
            .strings
            .iter()
            .map(|&pitch| harmonicon_core::midi::midi_to_note(i32::from(pitch)))
            .collect::<Vec<_>>()
            .join(" · "),
        Some(Instrument::Piano(piano)) => String::from(
            loc.msg_args("instrument-check-chart-piano", &[("keys", piano.keys.to_string())]),
        ),
        None => String::new(),
    };
    for mut label in &mut harp_labels {
        if label.0 != written_for {
            label.0.clone_from(&written_for);
        }
    }

    let text = match &choice.played {
        Some(played) if chart.harmonica.is_none() => substitution_message(chart, played, &loc),
        _ => {
            let baseline = cost_of(chart, &baseline_choice(chart));
            cost_message(&cost_of(chart, &choice), &baseline, &loc)
        }
    };
    for mut label in &mut labels {
        if label.0 != text {
            label.0.clone_from(&text);
        }
    }
}

/// Forgets the previous song's answer, so each visit re-seeds from its own
/// chart instead of inheriting a harp chosen for something else.
pub(crate) fn reset_harp_choice(mut choice: ResMut<HarpChoice>) {
    *choice = HarpChoice::default();
}

/// Where the track picker goes once there's a manifest to build it from.
#[derive(Component)]
pub(crate) struct TrackPickerSlot;

/// One track's label in the picker.
///
/// Names the part *and* how much of it a harmonica can actually play, since
/// the difference between a melody and a bass line is obvious in those
/// numbers and invisible in a name like "Track 4".
pub(crate) fn track_label(entry: &TrackChart, loc: &Localization) -> String {
    let name = entry.track.name.clone().unwrap_or_else(|| {
        loc.msg_args("harp-check-track-unnamed", &[("index", entry.track.index.to_string())])
            .to_string()
    });
    loc.msg_args(
        "harp-check-track-option",
        &[
            ("name", name),
            (
                "instrument",
                loc.msg(&format!(
                    "song-instrument-{}",
                    entry
                        .chart
                        .instrument()
                        .map_or(
                            harmonicon_core::instrument::InstrumentKind::Harmonica,
                            |instrument| instrument.kind()
                        )
                        .slug()
                ))
                .to_string(),
            ),
            ("notes", entry.report.total.to_string()),
            ("percent", ((entry.report.reachable_fraction() * 100.0).round() as u32).to_string()),
        ],
    )
    .to_string()
}

fn track_labels(tracks: &[TrackChart], loc: &Localization) -> Vec<String> {
    tracks.iter().map(|t| track_label(t, loc)).collect()
}

/// Builds the track picker once the manifest is readable.
///
/// Only for a file that genuinely posed a question: a `.harpchart` has one
/// part by construction and a score file with a single playable track had
/// nothing to choose between, so neither gets a control that can only be
/// set one way.
pub(crate) fn spawn_track_picker(
    mut commands: Commands,
    loc: Res<Localization>,
    selected: Option<Res<SelectedSong>>,
    manifests: Res<Assets<SongManifest>>,
    slots: Query<(Entity, Option<&Children>), With<TrackPickerSlot>>,
    page_root: Query<Entity, With<MenuRoot>>,
) {
    let Ok((slot, children)) = slots.single() else {
        return;
    };
    // Already built — this runs every frame precisely because the manifest
    // may arrive several frames after the page did.
    if children.is_some_and(|c| !c.is_empty()) {
        return;
    }
    let (Some(manifest), Ok(backdrop_parent)) =
        (selected.as_ref().and_then(|s| manifests.get(&s.0)), page_root.single())
    else {
        return;
    };
    if manifest.source_tracks.len() < 2 {
        return;
    }

    let options = track_labels(&manifest.source_tracks, &loc);
    let current = manifest.source_track.and_then(|i| options.get(i)).unwrap_or(&options[0]);

    combobox::spawn_combobox(
        &mut commands,
        slot,
        backdrop_parent,
        &loc.msg("harp-check-track"),
        &options,
        current,
        on_track_selected,
    );
}

/// Swaps the chosen part into the manifest.
///
/// Mutating the loaded asset rather than re-loading it with the choice
/// attached: every part was already converted at load time, so this is a
/// clone, and nothing downstream needs to learn that a song can have
/// alternates — gameplay keeps reading `manifest.chart`. The previous
/// choice isn't lost, since `source_tracks` still holds every part.
fn on_track_selected(
    ev: On<combobox::ComboboxSelect>,
    selected: Option<Res<SelectedSong>>,
    mut manifests: ResMut<Assets<SongManifest>>,
    mut choice: ResMut<HarpChoice>,
) {
    let Some(selected) = selected.as_ref() else {
        return;
    };
    let index = ev.index;
    let Some(manifest) = manifests.get(&selected.0) else {
        return;
    };
    if index >= manifest.source_tracks.len() || manifest.source_track == Some(index) {
        return;
    }
    let Some(mut manifest) = manifests.get_mut(&selected.0) else {
        return;
    };
    manifest.chart = manifest.source_tracks[index].chart.clone();
    manifest.source_track = Some(index);
    // Each part was fitted to its own harmonica, so the harp shown for the
    // old one is now wrong. Re-seeding is what `refresh_harp_cost` does
    // whenever a choice hasn't been made against the current chart.
    choice.seeded = false;
}

#[cfg(test)]
mod tests;

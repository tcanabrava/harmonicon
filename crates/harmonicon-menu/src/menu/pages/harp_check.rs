// SPDX-License-Identifier: MIT

//! "Which harmonica are you holding?" — shown after picking a song, before
//! it loads.
//!
//! A chart names the harp it was written for, and until this page existed
//! that was a hard requirement: own a G and a C-harp chart was simply
//! unplayable. Here the player can say what they actually have, and what
//! swapping should preserve — see `harmonicon_core::harp_remap`.
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
use bevy_fluent::Localization;

use harmonicon_app::app::{AppState, EffectiveHarmonica, SelectedSong};
use harmonicon_core::chart::HarpChart;
use harmonicon_core::harmonica::{Harmonica, detected_harp_key};
use harmonicon_core::harp_remap::{HarpMapping, RemapCost, remap_event};
use harmonicon_core::pitch_map::{HARP_KEYS, HarpKind, harp_for_key};
use harmonicon_platform::localization::LocalizationExt;
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
        let same_kind = matches!(
            (&chart.harmonica, self.kind),
            (Harmonica::Diatonic { .. }, HarpKind::Diatonic)
                | (Harmonica::Chromatic { .. }, HarpKind::Chromatic)
        );
        same_kind && detected_harp_key(&chart.harmonica).as_deref() == Some(self.key.as_str())
    }
}

/// The line reporting what the current choice will demand.
#[derive(Component)]
pub(crate) struct HarpCostLabel;

/// The line naming the harmonica the chart was written for.
#[derive(Component)]
pub(crate) struct ChartHarpLabel;

fn kind_label(kind: HarpKind, loc: &Localization) -> String {
    String::from(match kind {
        HarpKind::Diatonic => loc.msg("harp-kind-diatonic"),
        HarpKind::Chromatic => loc.msg("harp-kind-chromatic"),
    })
}

fn kind_labels(loc: &Localization) -> Vec<String> {
    vec![
        kind_label(HarpKind::Diatonic, loc),
        kind_label(HarpKind::Chromatic, loc),
    ]
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
    String::from(loc.msg_args(
        "harp-check-chart-harp",
        &[("key", key), ("kind", kind_label(kind, loc))],
    ))
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
    for item in &chart.track {
        for event in &item.events {
            let modifiers = event.modifiers.clone().unwrap_or_default();
            cost.add(&remap_event(
                event.hole,
                event.action,
                event.note.as_deref(),
                &modifiers,
                &chart.harmonica,
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
    seed_choice(&mut c, chart);
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
        parts.push(String::from(loc.msg_args(
            "harp-check-cost-bends",
            &[("count", added_bends.to_string())],
        )));
    }
    if added_overblows > 0 {
        parts.push(String::from(loc.msg_args(
            "harp-check-cost-overblows",
            &[("count", added_overblows.to_string())],
        )));
    }
    if cost.unplayable > 0 {
        parts.push(String::from(loc.msg_args(
            "harp-check-cost-unreachable",
            &[("count", cost.unplayable.to_string())],
        )));
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
) {
    // Seed from the chart when it's already resident (the common case: the
    // song list holds a strong handle, so it is usually decoded by now).
    // When it isn't, `refresh_harp_cost` seeds on the frame it arrives.
    if let Some(chart) = selected
        .as_ref()
        .and_then(|s| manifests.get(&s.0))
        .map(|m| &m.chart)
    {
        seed_choice(&mut choice, chart);
    }

    let (root, header, page_root) = spawn_menu_root(
        &mut commands,
        &loc.msg("harp-check-title"),
        None,
        &theme,
        "HarpCheck",
    );

    let intro = commands
        .spawn((
            Text::new(loc.msg("harp-check-intro")),
            TextFont {
                font_size: FontSize::Px(17.0),
                ..default()
            },
            TextColor(Color::srgb(0.82, 0.82, 0.88)),
            Node {
                max_width: Val::Px(560.0),
                ..default()
            },
        ))
        .id();
    commands.entity(root).add_child(intro);

    let written_for = commands
        .spawn((
            Text::new(""),
            TextFont {
                font_size: FontSize::Px(18.0),
                ..default()
            },
            TextColor(Color::srgb(0.95, 0.95, 1.0)),
            ChartHarpLabel,
        ))
        .id();
    commands.entity(root).add_child(written_for);

    combobox::spawn_combobox(
        &mut commands,
        root,
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
        root,
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
    let current_mapping =
        mapping_options[usize::from(choice.mapping == HarpMapping::Transpose)].clone();
    combobox::spawn_combobox(
        &mut commands,
        root,
        page_root,
        &loc.msg("harp-check-mapping"),
        &mapping_options,
        &current_mapping,
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
        .spawn((
            Text::new(""),
            TextFont {
                font_size: FontSize::Px(16.0),
                ..default()
            },
            TextColor(Color::srgb(0.95, 0.8, 0.45)),
            Node {
                max_width: Val::Px(560.0),
                ..default()
            },
            HarpCostLabel,
        ))
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
            let chart = selected
                .as_ref()
                .and_then(|s| manifests.get(&s.0))
                .map(|m| &m.chart);
            // Leave the default alone when nothing was actually changed, so
            // an untouched song stays on gameplay's original path.
            effective.harp = match chart {
                Some(chart) if choice.matches_chart(chart) => None,
                _ => Some(choice.harp()),
            };
            effective.mapping = choice.mapping;
            state.set(AppState::SongLoading);
        },
    );

    spawn_back_button(
        &mut commands,
        header,
        &loc.msg("back"),
        |_: On<Activate>, mut page: ResMut<NextState<MenuPage>>| page.set(MenuPage::SongList),
    );
}

/// Points `choice` at whatever the chart asks for.
fn seed_choice(choice: &mut HarpChoice, chart: &HarpChart) {
    if let Some(key) = detected_harp_key(&chart.harmonica) {
        choice.key = key;
    }
    choice.kind = match chart.harmonica {
        Harmonica::Chromatic { .. } => HarpKind::Chromatic,
        Harmonica::Diatonic { .. } => HarpKind::Diatonic,
    };
    choice.mapping = HarpMapping::default();
    choice.seeded = true;
}

/// Recomputes the cost line whenever the choice changes — or when the chart
/// finally decodes, which may be after this page was built.
pub(crate) fn refresh_harp_cost(
    mut choice: ResMut<HarpChoice>,
    loc: Res<Localization>,
    selected: Option<Res<SelectedSong>>,
    manifests: Res<Assets<SongManifest>>,
    mut labels: Query<&mut Text, With<HarpCostLabel>>,
    mut harp_labels: Query<&mut Text, (With<ChartHarpLabel>, Without<HarpCostLabel>)>,
) {
    let Some(chart) = selected
        .as_ref()
        .and_then(|s| manifests.get(&s.0))
        .map(|m| m.chart.clone())
    else {
        return;
    };
    if !choice.seeded {
        seed_choice(&mut choice, &chart);
    } else if !choice.is_changed() {
        return;
    }
    let written_for = harp_name(&chart.harmonica, &loc);
    for mut label in &mut harp_labels {
        *label = Text::new(written_for.clone());
    }

    let baseline = cost_of(&chart, &baseline_choice(&chart));
    let text = cost_message(&cost_of(&chart, &choice), &baseline, &loc);
    for mut label in &mut labels {
        *label = Text::new(text.clone());
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
        loc.msg_args(
            "harp-check-track-unnamed",
            &[("index", entry.track.index.to_string())],
        )
        .to_string()
    });
    loc.msg_args(
        "harp-check-track-option",
        &[
            ("name", name),
            ("notes", entry.report.total.to_string()),
            (
                "percent",
                ((entry.report.reachable_fraction() * 100.0).round() as u32).to_string(),
            ),
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
    let (Some(manifest), Ok(backdrop_parent)) = (
        selected.as_ref().and_then(|s| manifests.get(&s.0)),
        page_root.single(),
    ) else {
        return;
    };
    if manifest.source_tracks.len() < 2 {
        return;
    }

    let options = track_labels(&manifest.source_tracks, &loc);
    let current = manifest
        .source_track
        .and_then(|i| options.get(i))
        .cloned()
        .unwrap_or_else(|| options[0].clone());

    combobox::spawn_combobox(
        &mut commands,
        slot,
        backdrop_parent,
        &loc.msg("harp-check-track"),
        &options,
        &current,
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
    loc: Res<Localization>,
    selected: Option<Res<SelectedSong>>,
    mut manifests: ResMut<Assets<SongManifest>>,
    mut choice: ResMut<HarpChoice>,
) {
    let Some(mut manifest) = selected.as_ref().and_then(|s| manifests.get_mut(&s.0)) else {
        return;
    };
    // Matched against labels rebuilt from the same function that produced
    // them, so this keeps working in any locale.
    let Some(index) = track_labels(&manifest.source_tracks, &loc)
        .iter()
        .position(|label| *label == ev.value)
    else {
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

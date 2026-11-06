// SPDX-License-Identifier: MIT

//! The Options page's "My instruments" section: the guitar's tuning and
//! capo, the flute's foot joint and how a flute part is drawn, all persisted in
//! `settings::MyInstruments`. The pre-song page reads and writes the same
//! resource, so the answer given in either place is the one the other shows.
//!
//! Every combobox here speaks in localized labels, so each has a pure
//! label→value inverse resolved against the same `Localization` the list
//! was built from, which keeps it working in any locale.

use bevy::prelude::*;

use harmonicon_core::instrument::flute::FootJoint;
use harmonicon_core::instrument::guitar::TUNINGS;
use harmonicon_platform::localization::{Localization, LocalizationExt};
use harmonicon_platform::settings::{FluteView, MyInstruments};
use harmonicon_ui::dialogs::combobox;

use crate::menu::pages::harp_check::{capo_from_index, capo_index, capo_options};
use harmonicon_ui::dialogs::tooltip::Tooltip;

const FEET: [FootJoint; 2] = [FootJoint::C, FootJoint::B];
const VIEWS: [FluteView; 2] = [FluteView::Fingering, FluteView::Notes];

fn follow_chart(loc: &Localization) -> String {
    String::from(loc.msg("options-follow-chart"))
}

/// "As each song is written", then every named tuning.
pub(super) fn tuning_options(loc: &Localization) -> Vec<String> {
    std::iter::once(follow_chart(loc))
        .chain(TUNINGS.iter().map(|t| String::from(loc.msg(&format!("guitar-tuning-{}", t.id)))))
        .collect()
}

/// The tuning id an option names; `None` for "as each song is written".
pub(super) fn tuning_from_index(index: usize) -> Option<String> {
    index.checked_sub(1).and_then(|i| TUNINGS.get(i)).map(|t| t.id.to_owned())
}

/// Where `tuning` sits in [`tuning_options`]. An id no tuning has any more
/// shows as "as each song is written", which is also how the pre-song page
/// treats it.
pub(super) fn tuning_index(tuning: Option<&str>) -> usize {
    tuning.and_then(|id| TUNINGS.iter().position(|t| t.id == id)).map_or(0, |i| i + 1)
}

fn foot_key(foot: FootJoint) -> &'static str {
    match foot {
        FootJoint::C => "flute-foot-c",
        FootJoint::B => "flute-foot-b",
    }
}

/// "As each song is written", then each foot joint.
pub(super) fn foot_options(loc: &Localization) -> Vec<String> {
    std::iter::once(follow_chart(loc))
        .chain(FEET.iter().map(|f| String::from(loc.msg(foot_key(*f)))))
        .collect()
}

pub(super) fn foot_from_index(index: usize) -> Option<FootJoint> {
    index.checked_sub(1).and_then(|i| FEET.get(i)).copied()
}

pub(super) fn foot_index(foot: Option<FootJoint>) -> usize {
    foot.and_then(|f| FEET.iter().position(|x| *x == f)).map_or(0, |i| i + 1)
}

fn view_key(view: FluteView) -> &'static str {
    match view {
        FluteView::Fingering => "options-flute-view-fingering",
        FluteView::Notes => "options-flute-view-notes",
    }
}

pub(super) fn view_options(loc: &Localization) -> Vec<String> {
    VIEWS.iter().map(|v| String::from(loc.msg(view_key(*v)))).collect()
}

/// A heading and the three pickers, under `parent`.
pub(super) fn spawn_my_instruments(
    commands: &mut Commands,
    parent: Entity,
    page_root: Entity,
    loc: &Localization,
    mine: &MyInstruments,
) {
    let heading = commands
        .spawn_empty()
        .apply_scene(bsn! {
            Text({String::from(loc.msg("options-my-instruments"))})
            TextFont { font_size: {FontSize::Px(20.0)} }
            TextColor({Color::srgb(0.95, 0.95, 1.0)})
            Node { margin: {UiRect::top(Val::Px(12.0))} }
        })
        .insert(Tooltip(String::from(loc.msg("options-my-instruments-tooltip"))))
        .id();
    commands.entity(parent).add_child(heading);

    let tunings = tuning_options(loc);
    combobox::spawn_combobox(
        commands,
        parent,
        page_root,
        &loc.msg("options-guitar-tuning"),
        &tunings,
        &tunings[tuning_index(mine.guitar_tuning.as_deref())],
        |ev: On<combobox::ComboboxSelect>, mut mine: ResMut<MyInstruments>| {
            mine.guitar_tuning = tuning_from_index(ev.index);
        },
    );

    for kind in [
        harmonicon_core::instrument::fretted_strings::FrettedKind::Bass,
        harmonicon_core::instrument::fretted_strings::FrettedKind::Cavaquinho,
    ] {
        let setup = kind.default_setup();
        let presets = setup.tuning_presets();
        let options: Vec<_> = std::iter::once(follow_chart(loc))
            .chain(
                presets
                    .iter()
                    .map(|preset| loc.msg(&format!("guitar-tuning-{}", preset.id)).to_string()),
            )
            .collect();
        let selected = mine
            .fretted_tuning(kind)
            .and_then(|id| presets.iter().position(|preset| preset.id == id))
            .map_or(0, |index| index + 1);
        combobox::spawn_combobox(
            commands,
            parent,
            page_root,
            &loc.msg(&format!("options-{}-tuning", kind.instrument_kind().slug())),
            &options,
            &options[selected],
            move |ev: On<combobox::ComboboxSelect>, mut mine: ResMut<MyInstruments>| {
                let picked = ev
                    .index
                    .checked_sub(1)
                    .and_then(|index| presets.get(index))
                    .map(|preset| preset.id.to_owned());
                mine.set_fretted_tuning(kind, picked);
            },
        );
    }
    let capos = capo_options(follow_chart(loc), loc);
    combobox::spawn_combobox(
        commands,
        parent,
        page_root,
        &loc.msg("options-guitar-capo"),
        &capos,
        &capos[capo_index(mine.guitar_capo)],
        |ev: On<combobox::ComboboxSelect>, mut mine: ResMut<MyInstruments>| {
            mine.guitar_capo = capo_from_index(ev.index);
        },
    );

    let feet = foot_options(loc);
    combobox::spawn_combobox(
        commands,
        parent,
        page_root,
        &loc.msg("options-flute-foot"),
        &feet,
        &feet[foot_index(mine.flute_foot)],
        |ev: On<combobox::ComboboxSelect>, mut mine: ResMut<MyInstruments>| {
            mine.flute_foot = foot_from_index(ev.index);
        },
    );

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
        commands,
        parent,
        page_root,
        &loc.msg("clarinet-setup"),
        &clarinets,
        &clarinets[index],
        |ev: On<combobox::ComboboxSelect>, mut mine: ResMut<MyInstruments>| {
            mine.clarinet_key = match ev.index {
                1 => Some(harmonicon_core::instrument::clarinet::ClarinetKey::Bb),
                2 => Some(harmonicon_core::instrument::clarinet::ClarinetKey::A),
                _ => None,
            };
        },
    );

    let views = view_options(loc);
    let current = VIEWS.iter().position(|v| *v == mine.flute_view).unwrap_or(0);
    combobox::spawn_combobox(
        commands,
        parent,
        page_root,
        &loc.msg("options-flute-view"),
        &views,
        &views[current],
        |ev: On<combobox::ComboboxSelect>, mut mine: ResMut<MyInstruments>| {
            if let Some(view) = VIEWS.get(ev.index) {
                mine.flute_view = *view;
            }
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tuning_option_reads_back_as_the_tuning_it_names() {
        assert_eq!(tuning_from_index(0), None, "the first follows each song");
        for (i, tuning) in TUNINGS.iter().enumerate() {
            assert_eq!(tuning_from_index(i + 1).as_deref(), Some(tuning.id));
            assert_eq!(tuning_index(Some(tuning.id)), i + 1);
        }
        assert_eq!(tuning_index(None), 0);
        assert_eq!(tuning_index(Some("no-such-tuning")), 0);
        assert_eq!(tuning_from_index(TUNINGS.len() + 1), None);
    }

    #[test]
    fn every_capo_option_reads_back_as_the_capo_it_names() {
        use crate::menu::pages::harp_check::MAX_CAPO;
        assert_eq!(capo_from_index(0), None, "the first follows each song");
        for capo in 0..=MAX_CAPO {
            assert_eq!(capo_from_index(capo_index(Some(capo))), Some(capo));
        }
        assert_eq!(capo_index(None), 0);
        assert_eq!(capo_index(Some(MAX_CAPO + 1)), 0, "out of range follows each song");
        assert_eq!(capo_from_index(usize::from(MAX_CAPO) + 2), None);
    }

    #[test]
    fn every_foot_option_reads_back_as_the_foot_it_names() {
        assert_eq!(foot_from_index(0), None);
        for foot in FEET {
            assert_eq!(foot_from_index(foot_index(Some(foot))), Some(foot));
        }
        assert_eq!(foot_index(None), 0);
    }
}

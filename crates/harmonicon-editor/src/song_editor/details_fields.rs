// SPDX-License-Identifier: MIT

//! Registry of song-level rows in the Details form. The field behavior and
//! backing values remain in `state`/`meta_form`; this module only owns their
//! display order and localization keys.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Field {
    Tempo,
    Pickup,
    Key,
    Position,
    Music,
    Name,
    Author,
    Genre,
    Difficulty,
    SongFeel,
    Source,
    License,
    Description,
    PerfectWindow,
    GoodWindow,
    MissWindow,
    ComboEnabled,
    ComboBase,
    ComboStep,
    ComboMax,
    ComboDecay,
    LoopType,
    LoopRepeat,
    LoopStart,
    LoopEnd,
    Section,
    Chord,
    Groove,
    Lyric,
    LessonId,
    LessonUnit,
    LessonPath,
    LessonExplanation,
    LessonPrerequisites,
    LessonPassCriteria,
    LessonThreshold,
    LessonTechnique,
    LessonProgression,
    LessonScale,
}

impl Field {
    pub(super) fn is_cycle(self) -> bool {
        matches!(
            self,
            Self::Key
                | Self::Position
                | Self::Difficulty
                | Self::SongFeel
                | Self::ComboEnabled
                | Self::LoopType
                | Self::LoopRepeat
                | Self::LessonPassCriteria
                | Self::LessonTechnique
                | Self::LessonProgression
                | Self::LessonScale
                | Self::LessonPath
        )
    }
}

pub(super) const FIELDS: [(Field, &str); 25] = [
    (Field::Tempo, "editor-field-tempo"),
    (Field::Pickup, "editor-field-pickup"),
    (Field::Key, "editor-field-key"),
    (Field::Position, "editor-field-position"),
    (Field::Music, "editor-field-music"),
    (Field::Name, "editor-field-name"),
    (Field::Author, "editor-field-author"),
    (Field::Genre, "editor-field-genre"),
    (Field::Difficulty, "editor-field-difficulty"),
    (Field::SongFeel, "editor-field-feel"),
    (Field::Source, "editor-field-source"),
    (Field::License, "editor-field-license"),
    (Field::Description, "editor-field-description"),
    (Field::PerfectWindow, "editor-field-perfect-window"),
    (Field::GoodWindow, "editor-field-good-window"),
    (Field::MissWindow, "editor-field-miss-window"),
    (Field::ComboEnabled, "editor-field-combo-enabled"),
    (Field::ComboBase, "editor-field-combo-base"),
    (Field::ComboStep, "editor-field-combo-step"),
    (Field::ComboMax, "editor-field-combo-max"),
    (Field::ComboDecay, "editor-field-combo-decay"),
    (Field::LoopType, "editor-field-loop-type"),
    (Field::LoopRepeat, "editor-field-loop-repeat"),
    (Field::LoopStart, "editor-field-loop-start"),
    (Field::LoopEnd, "editor-field-loop-end"),
];

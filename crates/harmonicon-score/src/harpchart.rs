// SPDX-License-Identifier: MIT

//! Harmonicon's own `.harpchart`, behind [`ScoreFile`].
//!
//! Reading our native format through the same trait as everything else
//! looks redundant and isn't. A trait with one real implementation and one
//! privileged special case drifts, because nothing forces the special case
//! to keep fitting; putting the format we control through the same door
//! means the shape stays exercised by the one file we can always fix.
//!
//! A chart is already harmonica-specific — it has holes and breath
//! directions — so this discards that and reports only pitches, like any
//! other format. Playing a `.harpchart` normally does *not* go through
//! here: gameplay loads it directly. This exists so a chart can be a
//! *source*, e.g. re-derived onto a different harmonica, alongside the
//! foreign formats.

use harmonicon_core::chart::HarpChart;
use harmonicon_core::harp_remap::source_pitch;

use crate::{ScoreError, ScoreFile, ScoreFormat, ScoreNote, ScoreTrack};

pub struct HarpChartScore {
    chart: HarpChart,
    tracks: Vec<ScoreTrack>,
}

impl HarpChartScore {
    pub fn parse(bytes: &[u8]) -> Result<Self, ScoreError> {
        let chart: HarpChart = serde_json::from_slice(bytes)
            .map_err(|e| ScoreError::Parse { format: "harpchart", detail: e.to_string() })?;
        Ok(Self::from_chart(chart))
    }

    pub fn from_chart(chart: HarpChart) -> Self {
        // A score file is read in performance order, like any MIDI file.
        let chart = harmonicon_core::repeats::expand(chart);
        // A chart has exactly one part. The track list exists so callers
        // need no special case, not because there is a choice to make.
        let note_count = chart.track.iter().map(|item| item.events.len()).sum();
        let tracks =
            vec![ScoreTrack { index: 0, name: Some(chart.song.title.clone()), note_count }];
        Self { chart, tracks }
    }

    pub fn chart(&self) -> &HarpChart {
        &self.chart
    }
}

impl ScoreFile for HarpChartScore {
    fn format(&self) -> ScoreFormat {
        ScoreFormat::HarpChart
    }

    fn title(&self) -> Option<&str> {
        Some(&self.chart.song.title)
    }

    fn tracks(&self) -> &[ScoreTrack] {
        &self.tracks
    }

    fn notes(&self, track: usize) -> Result<Vec<ScoreNote>, ScoreError> {
        if track != 0 {
            return Err(ScoreError::NoSuchTrack(track));
        }
        let mut notes = Vec::new();
        for item in &self.chart.track {
            // An item states either a time or a tick; `gameplay::notes`
            // has its own copy of this rule, but that crate sits above this
            // one and cannot be reached from here.
            let start = match (item.time, item.tick) {
                (Some(secs), _) => secs,
                (None, Some(tick)) => harmonicon_core::chart::tick_to_seconds(
                    tick,
                    self.chart.timing.resolution,
                    &self.chart.timing.tempo_map,
                ),
                (None, None) => continue,
            };
            for event in &item.events {
                // `source_pitch` rather than reading `event.note` directly:
                // the name is the *natural reed*, and a bend or an
                // over-technique moves the pitch off it. Getting this wrong
                // reports the unbent note, which is a different tune.
                let Some(midi) = source_pitch(
                    event.hole,
                    event.action,
                    event.note.as_deref(),
                    event.modifiers.as_deref().unwrap_or(&[]),
                    &self.chart.harmonica,
                ) else {
                    continue;
                };
                notes.push(ScoreNote { start_secs: start, duration_secs: item.duration, midi });
            }
        }
        notes.sort_by(|a, b| {
            a.start_secs.partial_cmp(&b.start_secs).unwrap_or(std::cmp::Ordering::Equal)
        });
        Ok(notes)
    }

    fn tempo_bpm(&self) -> f32 {
        self.chart.song.tempo_bpm
    }

    fn time_signature(&self) -> (u8, u8) {
        self.chart.song.time_signature.as_deref().and_then(parse_time_signature).unwrap_or((4, 4))
    }
}

/// `"6/8"` as `(6, 8)`, for reading a score file's own meter. This crate
/// sits below `harmonicon-ui`, so it can't use that crate's
/// `MusicScoreMeter`; the tuple is converted into one at the boundary.
pub fn parse_time_signature(text: &str) -> Option<(u8, u8)> {
    let (numerator, denominator) = text.split_once('/')?;
    Some((numerator.trim().parse().ok()?, denominator.trim().parse().ok()?))
}

#[cfg(test)]
mod tests;

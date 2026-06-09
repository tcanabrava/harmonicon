use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
pub struct HarpChart {
    pub metadata: Option<Metadata>,
    pub song: Song,
    pub timing: Timing,
    pub harmonica: Harmonica,
    pub track: Vec<TrackItem>,
    #[serde(rename = "loop")]
    pub loop_section: Option<LoopSection>,
    pub scoring: Scoring,
    pub fx_mapping: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Metadata {
    pub format_version: Option<String>,
    pub author: Option<String>,
    pub source: Option<String>,
    pub license: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Song {
    pub title: String,
    pub artist: String,
    pub tempo_bpm: f32,
    pub key: String,
    pub time_signature: Option<String>,
    pub difficulty: Difficulty,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Difficulty {
    Easy,
    Intermediate,
    Advanced,
    Expert,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Timing {
    pub resolution: u32,
    pub tempo_map: Vec<TempoPoint>,
    pub time_signature_map: Option<Vec<TimeSigPoint>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TempoPoint {
    pub tick: u64,
    pub bpm: f32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TimeSigPoint {
    pub tick: u64,
    pub time_signature: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Harmonica {
    Diatonic {
        holes: u8,
        bending_profile: BendingProfile,
        position: Option<String>,
        layout: Option<DiatonicLayout>,
    },
    Chromatic {
        holes: u8,
        position: Option<String>,
        layout: Option<ChromaticLayout>,
    },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BendingProfile {
    RichterStandard,
    CountryTuned,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DiatonicLayout {
    pub blow: Option<Vec<String>>,
    pub draw: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChromaticLayout {
    pub blow: Option<Vec<String>>,
    pub draw: Option<Vec<String>>,
    pub blow_slide: Option<Vec<String>>,
    pub draw_slide: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TrackItem {
    pub id: Option<String>,
    pub time: Option<f64>,
    pub tick: Option<u64>,
    pub duration: f64,
    pub phrase: Option<String>,
    pub groove: Option<String>,
    pub play_mode: Option<PlayMode>,
    pub events: Vec<NoteEvent>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PlayMode {
    Single,
    Chord,
    Split,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NoteEvent {
    pub hole: u8,
    pub action: Action,
    pub note: Option<String>,
    pub modifiers: Option<Vec<Modifier>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    Blow,
    Draw,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum Modifier {
    #[serde(rename = "bend")]
    Bend { semitones: f32, intensity: Option<f32> },
    #[serde(rename = "overblow")]
    Overblow,
    #[serde(rename = "overdraw")]
    Overdraw,
    #[serde(rename = "vibrato")]
    Vibrato { oscillation_hz: f32, intensity: Option<f32> },
    #[serde(rename = "wah-wah")]
    WahWah { oscillation_hz: f32, intensity: Option<f32> },
    #[serde(rename = "hold")]
    Hold { intensity: Option<f32> },
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoopSection {
    pub start_index: usize,
    pub end_index: usize,
    #[serde(rename = "type")]
    pub section_type: Option<LoopType>,
    pub repeat: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LoopType {
    Intro,
    Verse,
    Chorus,
    Bridge,
    Outro,
    Full,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Scoring {
    pub perfect_window_ms: u32,
    pub good_window_ms: u32,
    pub miss_window_ms: u32,
    pub combo: Option<Combo>,
    pub style_bonus: Option<HashMap<String, f32>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Combo {
    pub enabled: bool,
    pub base_multiplier: f32,
    pub step_multiplier: f32,
    pub max_multiplier: f32,
    pub decay_ms: Option<u32>,
}

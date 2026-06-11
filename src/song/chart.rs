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

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL_DIATONIC: &str = r#"{
        "song": { "title": "Test", "artist": "Tester", "tempo_bpm": 120.0, "key": "C", "difficulty": "easy" },
        "timing": { "resolution": 480, "tempo_map": [{"tick": 0, "bpm": 120.0}] },
        "harmonica": {
            "type": "diatonic", "holes": 10, "bending_profile": "richter_standard",
            "layout": {
                "blow": ["C4","E4","G4","C5","E5","G5","C6","E6","G6","C7"],
                "draw": ["D4","G4","B4","D5","F5","A5","B5","D6","F6","A6"]
            }
        },
        "track": [],
        "scoring": { "perfect_window_ms": 50, "good_window_ms": 100, "miss_window_ms": 130 }
    }"#;

    #[test]
    fn minimal_chart_deserializes() {
        let chart: HarpChart = serde_json::from_str(MINIMAL_DIATONIC).unwrap();
        assert_eq!(chart.song.title, "Test");
        assert_eq!(chart.song.tempo_bpm, 120.0);
        assert_eq!(chart.scoring.perfect_window_ms, 50);
        assert_eq!(chart.scoring.good_window_ms, 100);
        assert!(chart.track.is_empty());
        assert!(chart.scoring.combo.is_none());
    }

    #[test]
    fn diatonic_layout_fields_parsed() {
        let chart: HarpChart = serde_json::from_str(MINIMAL_DIATONIC).unwrap();
        let Harmonica::Diatonic { holes, layout: Some(ref l), .. } = chart.harmonica else {
            panic!("expected Diatonic with layout");
        };
        assert_eq!(holes, 10);
        let blow = l.blow.as_ref().unwrap();
        assert_eq!(blow.len(), 10);
        assert_eq!(blow[0], "C4");
        assert_eq!(blow[9], "C7");
    }

    #[test]
    fn chromatic_harmonica_deserializes() {
        let json = r#"{
            "song": {"title":"T","artist":"A","tempo_bpm":120.0,"key":"C","difficulty":"easy"},
            "timing": {"resolution":480,"tempo_map":[{"tick":0,"bpm":120.0}]},
            "harmonica": {
                "type": "chromatic", "holes": 12,
                "layout": {
                    "blow":       ["C4","D4","E4","F4","G4","A4","B4","C5","D5","E5","F5","G5"],
                    "draw":       ["D4","E4","F#4","G4","A4","B4","C#5","D5","E5","F#5","G5","A5"],
                    "blow_slide": ["C#4","D#4","F4","F#4","G#4","A#4","B4","C#5","D#5","F5","F#5","G#5"],
                    "draw_slide": ["D#4","F4","G4","G#4","A#4","C5","D5","D#5","F5","G5","G#5","A#5"]
                }
            },
            "track": [],
            "scoring": {"perfect_window_ms":50,"good_window_ms":100,"miss_window_ms":130}
        }"#;
        let chart: HarpChart = serde_json::from_str(json).unwrap();
        assert!(matches!(chart.harmonica, Harmonica::Chromatic { holes: 12, .. }));
    }

    #[test]
    fn track_item_with_blow_event_parsed() {
        let json = r#"{
            "song": {"title":"T","artist":"A","tempo_bpm":120.0,"key":"C","difficulty":"easy"},
            "timing": {"resolution":480,"tempo_map":[{"tick":0,"bpm":120.0}]},
            "harmonica": {"type":"diatonic","holes":10,"bending_profile":"richter_standard"},
            "track": [{"time": 1.0, "duration": 0.5, "events": [{"hole": 4, "action": "blow"}]}],
            "scoring": {"perfect_window_ms":50,"good_window_ms":100,"miss_window_ms":130}
        }"#;
        let chart: HarpChart = serde_json::from_str(json).unwrap();
        assert_eq!(chart.track.len(), 1);
        let ev = &chart.track[0].events[0];
        assert_eq!(ev.hole, 4);
        assert!(matches!(ev.action, Action::Blow));
    }

    #[test]
    fn combo_scoring_config_parsed() {
        let json = r#"{
            "song": {"title":"T","artist":"A","tempo_bpm":120.0,"key":"C","difficulty":"easy"},
            "timing": {"resolution":480,"tempo_map":[{"tick":0,"bpm":120.0}]},
            "harmonica": {"type":"diatonic","holes":10,"bending_profile":"richter_standard"},
            "track": [],
            "scoring": {
                "perfect_window_ms": 40,
                "good_window_ms": 80,
                "miss_window_ms": 120,
                "combo": {
                    "enabled": true,
                    "base_multiplier": 1.0,
                    "step_multiplier": 0.25,
                    "max_multiplier": 4.0,
                    "decay_ms": 2000
                }
            }
        }"#;
        let chart: HarpChart = serde_json::from_str(json).unwrap();
        let combo = chart.scoring.combo.unwrap();
        assert!(combo.enabled);
        assert_eq!(combo.step_multiplier, 0.25);
        assert_eq!(combo.decay_ms, Some(2000));
    }

    #[test]
    fn difficulty_variants_all_parse() {
        for (s, _) in &[
            ("easy", "easy"), ("intermediate", "intermediate"),
            ("advanced", "advanced"), ("expert", "expert"),
        ] {
            let json = format!(r#"{{
                "song": {{"title":"T","artist":"A","tempo_bpm":120.0,"key":"C","difficulty":"{s}"}},
                "timing": {{"resolution":480,"tempo_map":[{{"tick":0,"bpm":120.0}}]}},
                "harmonica": {{"type":"diatonic","holes":10,"bending_profile":"richter_standard"}},
                "track": [],
                "scoring": {{"perfect_window_ms":50,"good_window_ms":100,"miss_window_ms":130}}
            }}"#);
            serde_json::from_str::<HarpChart>(&json)
                .unwrap_or_else(|e| panic!("difficulty '{s}' failed to parse: {e}"));
        }
    }
}

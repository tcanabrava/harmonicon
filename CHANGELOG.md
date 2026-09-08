# Changelog

## v0.0.11 — 2026-09-08

- Drop the chromatic-range item, now fixed
- Give the chromatic harmonica a real layout
- Widen the staff for songs that need it
- Make accidentals follow the bar, not the note
- Update the format gap now that six more formats load
- Play Guitar Pro, MuseScore and MusicXML files
- Record what actually blocks Guitar Pro support
- Stop suggested_harp claiming a chromatic plays everything
- Load score files through the trait, and let the player pick the part
- Open a MIDI import on the harmonica track, not track 0
- Play a MIDI file as a song, through harmonicon-score
- Add harmonicon-score: every score format behind one trait
- Sweep stale build artifacts on every commit
- Document cargo-sweep as the target/ maintenance step
- Cut the debug binary from 2410 MB to 287 MB
- Stop debug builds from filling the disk
- Name the harmonica a chart was written for, and fix two bugs it exposed
- Ask which harmonica the player is holding before a song starts
- Play a chart on a substituted harmonica, end to end through scoring
- Add harp_remap, and fix overblows being invisible to the microphone
- Move pitch→hole resolution into core, and teach it overblows
- Plan harmonica choice and multi-format score import
- Update libraries

## v0.0.10 — 2026-08-31

- release.sh: stop --dry-run discarding a hand-edited version
- release.sh: allow a version-only dirty tree, and write a changelog
- Fix the Android CI job failing whenever the cache is restored
- Add scripts/release.sh to cut a release in one command
- Warn when the chosen detector cannot hear a chart's chords
- Pin what each pitch algorithm can actually hear of a chord
- Report a microphone unplugged mid-session instead of going quietly deaf
- Reconcile the version, and make the tag/manifest agreement a CI gate
- Correct the 1.0 unwrap-triage figure: it was a miscount
- Say so during play when the microphone isn't working
- Greet a first-time player instead of dropping them on the main menu
- Define 1.0 as a readiness gate, scoped to desktop
- Read font cmaps with skrifa, not the unmaintained ttf-parser

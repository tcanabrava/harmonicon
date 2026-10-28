# harmonicon-app

App-wide vocabulary every feature shares: the state machine, cross-state
routing flags, and the player's persisted records.

Deliberately tiny and free of any feature — anything only one feature
needs belongs in that feature instead.

Project-wide rules (workspace layering, localization, testing style,
commit conventions) are in the root `CLAUDE.md` — this file is only what's
load-bearing about *this* crate.

## Architecture (load-bearing facts)

- **States:** `AppState` (Startup/Menu/SongLoading/Playing/Results/
  Calibration/Credits/SongEditor2/BendingTrainer) + `MenuPage` sub-states in
  `menu/mod.rs`. `GameplayMode` (Play2D/JamSession) selects which
  setup/update chains run within `Playing`.

- **Profile:** `<config>/harmonicon/profile.json` (`profile.rs`) — per-song
  best score/accuracy, per-technique best accuracy, bend-trainer drill
  records, total play time. Unlike settings it saves directly at the
  (infrequent) points where a record changes, plus a flush on `AppExit` for
  play time — deliberately no debounce machinery; keep new fields on that
  pattern.

- **Training reviews are dated in whole days**, from
  `harmonicon_platform::calendar::today()` — the game's only wall-clock
  read, through `web_time` because `std::time::SystemTime::now()` panics
  on wasm. Keep any new date on that function rather than calling
  `SystemTime` directly. `record_training` takes the day as an argument so
  the schedule stays pure and testable.

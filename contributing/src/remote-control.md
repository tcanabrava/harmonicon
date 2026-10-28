# Driving a Running Game from Outside

`--features dev` starts a [Bevy Remote Protocol][brp] server — JSON-RPC over
HTTP on `127.0.0.1:15702` — so a running game can be inspected, mutated,
screenshotted and recorded from a shell, with no rebuild and no code added
at each interesting moment. Wiring is `src/dev_capture.rs`.

**Never shipped.** BRP is unauthenticated and can read and mutate arbitrary
world state. `dev` is a compile-time feature, so a release build doesn't
merely disable it, it doesn't contain it.

[brp]: https://docs.rs/bevy_remote

## Running

```bash
cargo run --features dev
```

Launching the built binary *directly* needs one extra thing, or every asset
fails to load and you get a blank window: Bevy resolves `assets/` relative to
the **executable** (`target/debug/assets`), not the working directory, unless
`CARGO_MANIFEST_DIR` is set — which `cargo run` does and a bare `./harmonicon`
does not.

```bash
BEVY_ASSET_ROOT="$PWD" ./target/debug/harmonicon
```

(`configured_asset_plugin` in `src/lib.rs` special-cases this for macOS debug
builds only; everywhere else, use one of the two forms above.)

A helper for the examples below:

```bash
brp() { curl -s -X POST http://127.0.0.1:15702 \
        -H 'Content-Type: application/json' -d "$1"; }
```

For anything beyond a one-off poke, `scripts/brpctl.py` already wraps these
request shapes — including resolving a button from its label, and a
`--take-all-screenshots` tour that drives every screen at two window sizes.
The raw shapes are documented here anyway, because the wrapper is worth
reading only once you know what it is wrapping.

## Screenshots → `target/screenshots/`

Bevy's own `Screenshot` component is `Reflect` and registered, so BRP can
spawn one. `dev_capture`'s global observer writes whatever gets captured:

```bash
brp '{"jsonrpc":"2.0","id":1,"method":"world.spawn_entity","params":
     {"components":{"bevy_render::view::window::screenshot::Screenshot":
     {"Window":"Primary"}}}}'
```

Files are named `shot_<unix_millis>.png`, so repeated captures accumulate
rather than overwriting. Deliberately not bevy's `save_to_disk`, which takes
one fixed path.

## Video → `target/video/`

Set `frames_left`; one frame is captured per rendered frame until it reaches
zero. Each recording gets its own numbered directory.

```bash
brp '{"jsonrpc":"2.0","id":2,"method":"world.mutate_resources","params":
     {"resource":"harmonicon::dev_capture::VideoCapture",
      "path":".frames_left","value":300}}'
```

The game writes numbered PNGs; encoding is left outside, since pulling a
video encoder into the dependency tree for a dev tool isn't worth it:

```bash
ffmpeg -y -framerate 30 -i target/video/0001/frame_%06d.png \
       -c:v libx264 -pix_fmt yuv420p target/video/0001.mp4
```

**Capturing every frame stalls the render loop** — each frame is a GPU
readback. Expect the recorded clip to run slower than real time, and don't
use it to judge performance or timing. It shows *what* happened, not *how
fast*.

## Inspecting and mutating state

```bash
# What the menu actually says right now
brp '{"jsonrpc":"2.0","id":3,"method":"world.query","params":
     {"data":{"components":["bevy_ui::widget::text::Text"]},"filter":{}}}'

# Everything reachable
brp '{"jsonrpc":"2.0","id":4,"method":"world.list_components"}'
brp '{"jsonrpc":"2.0","id":5,"method":"world.list_resources"}'
brp '{"jsonrpc":"2.0","id":6,"method":"rpc.discover"}'
```

Type paths are the full Rust paths and they matter: UI text is
`bevy_ui::widget::text::Text`, **not** `bevy_text::text::Text` (which exists,
is registered, and matches nothing on a UI node).

**A component's path and its crate's `use`-visible name are not the same**,
and the two halves of the same widget don't even agree with each other. The
button *component* is registered under its declaring module,
`bevy_ui_widgets::button::Button`; the `Activate` *event* is registered as
`bevy_ui_widgets::Activate`, because that is the re-export `dev_capture`
hands to `register_type`. Guessing either from the other fails, and fails
differently: a wrong component path matches nothing and looks like an empty
screen, while a wrong event path at least answers `Unknown event type`.

Another thing a query cannot tell you: **whether any of this is on screen.**
The pause menu's buttons are spawned and merely hidden while a song plays, so
they are in every query result during normal play. See the "does not give
you" section below — this is the same blind spot as fonts.

`world.mutate_components`/`world.mutate_resources` change state live, and
`world.write_message`/`world.trigger_event` fire messages and events without
synthesising input — useful precisely where synthetic input is unreliable
(see [Building and Running on Android](android-build.md) on sub-frame taps
being dropped).

### The catch: only reflected, registered types are visible

BRP reaches everything through `AppTypeRegistry`. Bevy's own components are
registered, which covers the whole UI tree, text, transforms and windows.
**Most of this codebase's own types are not** — `EditorToolbar`,
`MicStatus`, `Scroll` and friends are plain `#[derive(Component)]`/
`Resource`, so `world.list_resources` shows only the handful that derive
`Reflect` and call `register_type` precisely so they can be driven from
outside: `VideoCapture`, `NextState<AppState>`/`NextState<MenuPage>`,
`LoopConfig` (an A–B range is otherwise a pointer drag) and
`gameplay::autoplay::Autoplay`.

**Autoplay is how a headless run gets hits.** With no microphone every note
is a miss, so nothing a hit shows — the head pop and ✓ stamp, the gold
tail while a note is held, live vibrato/wah confirmation, the results
screen's timing bar and Input-lag suggestion — was ever capturable. Switch
it on before entering a song and the game sounds each note itself, on the
judge's own timeline, with a wobble for a declared vibrato and a pumped
loudness for a wah; `late_ms` plays every attack behind the beat to make
the timing deliberately lopsided:

```bash
python3 scripts/brpctl.py autoplay on        # dead on
python3 scripts/brpctl.py autoplay on 40     # 40 ms late, every note
python3 scripts/brpctl.py autoplay off
```

It writes `ActivePitches`/`AudioFrame` right after `collect_pitches`, so the
judge is exercised unchanged; it is `#[cfg(feature = "dev")]` like the rest
of this. Turn it off before a screenshot that's meant to show a miss.

Add `#[derive(Reflect)]` + `app.register_type::<T>()` per type as the need
arises, rather than blanket-deriving it.

## Navigating: two ways, and when each one works

Screens that need no prior selection are one `NextState` write away:

```bash
brp '{"jsonrpc":"2.0","id":7,"method":"world.mutate_resources","params":
     {"resource":"bevy_state::state::resources::NextState<harmonicon_menu::menu::routing::MenuPage>",
      "path":"","value":{"Pending":"Options"}}}'
```

Note the exact path: `bevy_state::state::resources::NextState`, not
`…::states::NextState`. Swap `MenuPage` for
`harmonicon_app::app::AppState` (`"Calibration"`, `"SongEditor2"`,
`"BendingTrainer"`, …) for the screens outside the menu hierarchy.

**That is as far as state alone gets you.** Play 2D, Jam Session and
Results all need a `SelectedSong` first, which holds a `Handle<SongManifest>`
— not something a JSON value can express. So the other way is to *click the
button*, which is why `dev_capture` registers `bevy_ui_widgets::Activate`:

```bash
brp '{"jsonrpc":"2.0","id":8,"method":"world.trigger_event","params":
     {"event":"bevy_ui_widgets::Activate","value":{"entity":4294966678}}}'
```

`Activate` carries its own target, so the entity goes in the payload rather
than in a separate parameter. Since every click handler in this codebase is
an `On<Activate>` on a real `bevy_ui_widgets::Button` (see `CLAUDE.md`), one
trigger reaches any of them.

Finding the entity is the fiddly part. A label lives on a `Text` node that
may sit **several levels below** the entity carrying `Button` —
`dialogs/button.rs` wraps its content in a shell — so resolve it by walking
`bevy_ecs::hierarchy::ChildOf` upward until you hit an entity in the
`Button` set, rather than assuming the text's immediate parent is the
button.

One trap worth knowing: **query the text nodes and the hierarchy in as few
round trips as you can.** Menu pages despawn and respawn their whole subtree
on navigation, so an entity id read in one request can be gone by the next,
which looks exactly like "this button doesn't exist".

**A stale entity id can kill the game, not just fail.** `world.trigger_event`
on a despawned entity is harmless, but `world.insert_components` and
`world.mutate_components` panic inside `bevy_remote` itself — the panic
propagates through `process_remote_requests` and takes the process down:

```
thread 'main' panicked at bevy_remote-0.19.1/src/builtin_methods.rs:1194
Note that interacting with a despawned entity is the most common cause
```

Gameplay is worse than menus for this, because a scene can vanish on its own
schedule: a song that reaches its end tears down every `GameplayRoot` entity
and moves to `Results` with no input from you. Re-query immediately before
mutating, and don't assume an id you captured before a `sleep` is still
alive.

## What it's already used for

Every image in `docs/book/src/images/` is a real capture taken this way —
nine of the fifteen by setting `NextState` alone, the rest (gameplay, the
jam grid, the results screen, the tour overlay, the microphone dropdown) by
clicking through to them. If a screen changes, re-take that one PNG under
the same filename; the `![...](images/foo.png)` references don't move.

## Checking a translation

The game follows the OS locale, so a language is chosen at launch, not over
BRP. `scripts/locale_tour.py` then walks every menu page (by state), the
song list / harp check / a lesson (by clicking that locale's own labels,
resolved from its `.ftl`), a song with autoplay, the results screen, a jam
and the editor, writing named PNGs:

```bash
LANG=pt_BR.UTF-8 LC_ALL=pt_BR.UTF-8 ./scripts/run-dev.sh --no-build
LANG=pt_BR.UTF-8 LC_ALL=pt_BR.UTF-8 python3 scripts/locale_tour.py target/screenshots/pt-BR
```

A contact sheet of the output is the fastest way to spot an English label
or a wrapped one — the pass that added this found a dozen single-word
literals the localization lint's fingerprint deliberately skips.

## What this does *not* give you

- **Rendering correctness.** BRP reports the *string* `"♬ Import MIDI"`
  whether or not the font has the glyph. Five tofu boxes shipped in every
  locale for months and only a screenshot caught them. Use both.
- **Timing.** See the readback stall above.
- **Audio.**

## Android

The same server runs in the Android build (`--features dev`), reachable by
forwarding the port:

```bash
adb forward tcp:15702 tcp:15702
```

The capture directories are then inside the app's sandbox rather than
`target/`, so pull them with `adb`.

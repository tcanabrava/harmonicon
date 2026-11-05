// SPDX-License-Identifier: MIT

//! Dev-only screen capture, driven from *outside* the running game over the
//! Bevy Remote Protocol (`bevy_remote`, JSON-RPC on `127.0.0.1:15702`).
//!
//! The point of routing this through BRP rather than a keybind or a
//! frame-counter is that nothing about *when* to capture is baked into the
//! game. An external tool decides, at any moment, as often as it likes — and
//! can mutate world state first (`world.mutate_resources`) and capture the
//! result in the same breath, with no rebuild.
//!
//! Two entry points, both spawned from outside:
//!
//! - **A still:** spawn an entity with bevy's own `Screenshot` component
//!   (it is `Reflect` and registered by `ScreenshotPlugin`, so BRP's
//!   `world.spawn_entity` can create one). [`save_capture`] writes it to
//!   [`SCREENSHOT_DIR`].
//! - **A clip:** set [`VideoCapture::frames_left`] via
//!   `world.mutate_resources`. [`drive_video_capture`] then spawns one
//!   screenshot per frame until it runs out, numbering them into a
//!   per-recording directory under [`VIDEO_DIR`]. Encoding to an actual
//!   video file is left to `ffmpeg` outside the game — see
//!   `contributing/src/remote-control.md`.
//!
//! Navigation is the other half of this, and mostly needs no code here:
//! `NextState<AppState>`/`NextState<MenuPage>` are reflected, so
//! `world.mutate_resources` reaches any screen that needs no prior
//! selection. The ones that *do* need one — a song picked, a jam
//! generated, the guided tour started — are reached by clicking their
//! button, which is why this plugin registers `Activate`.
//!
//! Never shipped: BRP is an unauthenticated RPC server that can read and
//! mutate arbitrary world state, which is fine bound to localhost on a
//! developer's machine and nowhere else. Gated on `--features dev`, which
//! (unlike a runtime flag) keeps the whole module out of a release build.

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};
use std::path::{Path, PathBuf};

/// Where single screenshots land. Named by capture time, so repeated
/// captures accumulate instead of overwriting each other.
pub const SCREENSHOT_DIR: &str = "target/screenshots";

/// Where recorded clips land, one numbered subdirectory per recording.
pub const VIDEO_DIR: &str = "target/video";

/// An in-progress recording. Start one from outside with
/// `world.mutate_resources` on this resource, setting `frames_left`:
///
/// ```jsonc
/// {"method": "world.mutate_resources", "params": {
///   "resource": "harmonicon::dev_capture::VideoCapture",
///   "path": ".frames_left", "value": 300 }}
/// ```
///
/// `session`/`frame` are bookkeeping this module owns — overwriting them
/// from outside is harmless but pointless.
#[derive(Resource, Reflect, Default, Debug)]
#[reflect(Resource)]
pub struct VideoCapture {
    /// Frames still to capture. Zero means "not recording"; setting it
    /// non-zero starts a fresh recording on the next frame.
    pub frames_left: u32,
    /// Which recording directory the current clip is writing into.
    pub session: u32,
    /// Frame index within the current recording, 1-based.
    pub frame: u32,
}

/// Tags a screenshot entity as belonging to a recording rather than being a
/// one-off still, so [`save_capture`] knows which directory it belongs in.
/// Not `Reflect`: it is spawned only by [`drive_video_capture`], never from
/// outside.
#[derive(Component)]
struct VideoFrame {
    session: u32,
    index: u32,
}

pub struct DevCapturePlugin;

impl Plugin for DevCapturePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(bevy::remote::RemotePlugin::default())
            .add_plugins(bevy::remote::http::RemoteHttpPlugin::default())
            .init_resource::<VideoCapture>()
            // Without registering it, `world.mutate_resources` can't see the
            // resource at all — BRP reaches everything through
            // `AppTypeRegistry`.
            .register_type::<VideoCapture>()
            // `Activate` is what every button in this codebase listens for
            // (see the click-handler rule in `CLAUDE.md`), so registering it
            // makes `world.trigger_event` a remote *click* on any button —
            // which reaches the screens no amount of `NextState` juggling
            // can, because they need a selection first: picking a song,
            // starting a generated jam, starting the guided tour.
            //
            // It carries its own target entity, so the entity to click is
            // part of the payload rather than a separate parameter.
            // `bevy_ui_widgets` derives `Reflect` on it but never registers
            // it, so this is the only thing making it reachable — and it is
            // deliberately here, in the dev-only module, rather than
            // alongside the widget plumbing.
            .register_type::<bevy::ui_widgets::Activate>()
            // Radio-based tabs emit a value change rather than Activate.
            .register_type::<bevy::ui_widgets::ValueChange<Entity>>()
            // An A–B loop is set by dragging on the progress bar — a pointer
            // gesture remote control can't make — so the range is written
            // directly instead, to reach the loop UI for captures.
            .register_type::<harmonicon_gameplay::gameplay::LoopConfig>()
            // Plays the chart itself, so hits — and everything only a hit
            // shows: head pops, hold fills, the results timing bar — can be
            // captured without a microphone.
            .register_type::<harmonicon_gameplay::gameplay::autoplay::Autoplay>()
            .add_observer(save_capture)
            .add_systems(Update, drive_video_capture);
        info!("Bevy Remote Protocol listening on 127.0.0.1:15702 (--features dev)");
    }
}

/// Spawns one screenshot per frame while a recording is running.
///
/// `recording` tracks the rising edge of `frames_left` so a fresh
/// `world.mutate_resources` call starts a *new* numbered directory rather
/// than appending to the previous clip.
fn drive_video_capture(
    mut capture: ResMut<VideoCapture>,
    mut commands: Commands,
    mut recording: Local<bool>,
) {
    if capture.frames_left == 0 {
        *recording = false;
        return;
    }
    if !*recording {
        *recording = true;
        capture.session += 1;
        capture.frame = 0;
    }
    capture.frame += 1;
    capture.frames_left -= 1;
    let (session, index) = (capture.session, capture.frame);
    commands.spawn((Screenshot::primary_window(), VideoFrame { session, index }));
}

/// Writes any captured frame to disk — a global observer, so it fires for
/// screenshots spawned from outside over BRP just as much as for the ones
/// [`drive_video_capture`] spawns.
///
/// Deliberately *not* bevy's own `save_to_disk`, which takes one fixed path
/// and would therefore overwrite every previous capture.
fn save_capture(event: On<ScreenshotCaptured>, frames: Query<&VideoFrame>) {
    let path = match frames.get(event.entity) {
        Ok(frame) => {
            let dir = PathBuf::from(VIDEO_DIR).join(format!("{:04}", frame.session));
            dir.join(format!("frame_{:06}.png", frame.index))
        }
        Err(_) => {
            // Millisecond-resolution timestamp: unique across captures and
            // across runs, and sorts chronologically as a plain filename.
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0);
            PathBuf::from(SCREENSHOT_DIR).join(format!("shot_{stamp}.png"))
        }
    };
    write_png(&event.image, &path);
}

/// Converts a captured frame and writes it out, creating the directory on
/// the way. Mirrors what bevy's `save_to_disk` does with the image, dropping
/// the alpha channel — with HDR enabled it carries brightness rather than
/// transparency, and keeping it makes the file look wrong.
fn write_png(image: &Image, path: &Path) {
    let Some(parent) = path.parent() else { return };
    if let Err(err) = std::fs::create_dir_all(parent) {
        error!("Cannot create {}: {err}", parent.display());
        return;
    }
    match image.clone().try_into_dynamic() {
        Ok(dynamic) => match dynamic.to_rgb8().save(path) {
            Ok(()) => info!("Captured {}", path.display()),
            Err(err) => error!("Cannot write {}: {err}", path.display()),
        },
        Err(err) => error!("Captured frame is not a saveable image: {err:?}"),
    }
}

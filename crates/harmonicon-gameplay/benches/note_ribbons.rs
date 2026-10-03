// SPDX-License-Identifier: MIT

//! What `animate_note_ribbons` costs per frame on the main world.
//!
//! Every visible note's ribbon material carries the animation clock in its
//! own uniform (`shape.z`), so the animator mutably touches every ribbon
//! material each frame, and each touch queues an `AssetEvent::Modified`. This measures
//! that system inside a minimal app with the real asset plugin — so the
//! event queuing and flushing are part of the measured frame — against the
//! same app without it, at ribbon counts from a sparse screen to a stress case.
//!
//! **It cannot see the render world.** A modified material is also
//! re-extracted and its bind group re-prepared (a fresh uniform buffer on the
//! GPU) — likely the larger share, and one that needs a GPU device to time.
//! A Tracy capture of a real song (`contributing/src/profiling.md`) covers
//! that half; this bench answers whether the main-world half matters at all.
//!
//! Run with `cargo bench -p harmonicon-gameplay --bench note_ribbons`.

use std::hint::black_box;

use bevy::asset::AssetPlugin;
use bevy::prelude::*;
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};

use harmonicon_gameplay::gameplay::GameplayClock;
use harmonicon_gameplay::gameplay::note_ribbon_2d::{NoteRibbon2dMaterial, animate_note_ribbons};
use harmonicon_platform::settings::ReducedMotion;

/// A minimal app holding `ribbons` note-ribbon materials, with the animator
/// registered or not.
fn app(ribbons: usize, animate: bool) -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<NoteRibbon2dMaterial>()
        .insert_resource(GameplayClock::new(12.5))
        .insert_resource(ReducedMotion(false));
    if animate {
        app.add_systems(Update, animate_note_ribbons);
    }
    let mut materials = app.world_mut().resource_mut::<Assets<NoteRibbon2dMaterial>>();
    // Handles are kept alive for the app's lifetime, as spawned notes do.
    let handles: Vec<Handle<NoteRibbon2dMaterial>> = (0..ribbons)
        .map(|i| {
            materials.add(NoteRibbon2dMaterial {
                color: LinearRgba::rgb(0.3, 0.5, 1.0),
                technique: Vec4::new(2.0, 5.0, 0.0, 0.0),
                shape: Vec4::new(0.5 + i as f32 * 0.01, 26.0, 0.0, 0.0),
                hold: Vec4::ZERO,
            })
        })
        .collect();
    app.insert_resource(KeepAlive(handles));
    // Settle startup work (initial asset events) before timing frames.
    app.update();
    app.update();
    app
}

#[derive(Resource)]
struct KeepAlive(#[allow(dead_code)] Vec<Handle<NoteRibbon2dMaterial>>);

fn note_ribbon_frames(c: &mut Criterion) {
    let mut group = c.benchmark_group("note_ribbon_frame");
    for ribbons in [16usize, 64, 256] {
        for (label, animate) in [("without animator", false), ("with animator", true)] {
            let mut app = app(ribbons, animate);
            group.bench_with_input(BenchmarkId::new(label, ribbons), &ribbons, |b, _| {
                b.iter(|| {
                    app.update();
                    black_box(&app);
                });
            });
        }
    }
    group.finish();
}

criterion_group!(benches, note_ribbon_frames);
criterion_main!(benches);

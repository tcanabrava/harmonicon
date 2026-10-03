// SPDX-License-Identifier: MIT

//! Writes a synthesized benchmark dataset under `assets/debug_songs/` (or a
//! path given on the command line) so `note_bench` has something to run
//! against without a real harmonica/microphone take — see
//! `harmonicon_bench::synthetic_dataset` for what's generated and why this is a
//! stand-in, not a replacement, for real recordings.
//!
//! Usage: `cargo run --bin gen_synthetic_dataset [-- <out_dir>]`, `out_dir`
//! defaulting to `assets/debug_songs`.

use harmonicon_bench::synthetic_dataset::write_all;
use std::path::Path;

fn main() {
    let out_dir = std::env::args().nth(1).unwrap_or_else(|| "assets/debug_songs".to_string());
    let out_dir = Path::new(&out_dir);

    match write_all(out_dir) {
        Ok(written) => {
            if written.is_empty() {
                println!("Nothing written — no scenario produced any notes.");
                return;
            }
            println!("Wrote {} synthetic recording(s):", written.len());
            for dir in &written {
                println!("  {}", dir.display());
            }
            println!(
                "\nRun `cargo run --bin note_bench -- {}` to benchmark against them.",
                out_dir.display()
            );
        }
        Err(e) => {
            eprintln!("Failed to write synthetic dataset: {e}");
            std::process::exit(1);
        }
    }
}

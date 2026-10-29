---
name: add-crate
description: Add a new crate to the Harmonicon workspace, or move a module between existing crates. Use when extracting a subsystem, splitting an over-large crate, or when a move breaks include_str!/OUT_DIR/feature-unification. Encodes the traps found while doing the original eleven-crate split.
---

# Adding or extracting a workspace crate

Layer order is in the root `CLAUDE.md`. A crate may depend only on ones
below it, and **peers may not depend on each other**.

## Extract bottom-up

Move a module only once everything it depends on is already a crate. Then
the new crate never depends on the root package, and the binary never has
to move. Going top-down instead forces a package-level cycle: `main.rs` is
a bin target of the root package, bins share that package's
`[dependencies]`, so the package cannot depend on a crate that depends on
its own lib.

## Steps

1. `git mv src/<module> crates/harmonicon-<name>/src/<module>` — keep it a
   subdirectory unless the module *is* the crate, in which case its
   `mod.rs` becomes `lib.rs` and every `<module>/foo.rs` path in the docs
   loses a level.
2. Write `Cargo.toml`. **Copy dependency version specs verbatim from the
   root** — guessing produces two incompatible copies in the tree
   (`fluent 0.16` vs `0.17` did exactly that, and the error surfaces as an
   unsatisfied `Borrow` trait bound, not as a version complaint). Promote
   shared deps to `[workspace.dependencies]` and use
   `{ workspace = true }` in members. Note the trap: inside
   `[workspace.dependencies]` the entry needs the real version, *not*
   `workspace = true`.
3. **Forward all three features** or Bevy gets built twice with different
   features:
   ```toml
   [features]
   dev = ["bevy/file_watcher", "bevy/debug", "harmonicon-x/dev"]
   dynamic_linking = ["bevy/dynamic_linking", "harmonicon-x/dynamic_linking"]
   trace_tracy = ["bevy/trace_tracy", "harmonicon-x/trace_tracy"]
   ```
   Not `bevy/dev`: it brings `render_dev_tools`, which panics at startup
   without Bevy's `3d` feature (the root `Cargo.toml` says why). Add the new
   crate to all three of the root's lists too. `dynamic_linking`
   is deliberately separate from `dev`: it makes doctests unable to load the
   stdlib, so folding it back into `dev` would break `cargo test`.
4. Rewrite call sites to name the crate: `harmonicon_x::module::Item`.
   **Do not add a re-export facade** — it hides which crate code comes
   from, which is the whole point of the boundary.
5. Lift grouped imports. `use crate::{ a::X, b::Y };` needs the moved entry
   pulled out into its own `use harmonicon_x::a::X;`. A plain sed cannot do
   this; nested groups span lines.

## What breaks on a move

| Thing | Why | Fix |
|---|---|---|
| `include_str!`/`include_bytes!` | resolve relative to the *source file* | add the levels: `../../assets/…` → `../../../../assets/…`. A wrong path is a compile error, and the diagnostic suggests the right one. |
| `env!("CARGO_MANIFEST_DIR")` | now the crate, not the repo | `.join("../..")`. **Not** a compile error — verify the test still fails on a deliberately wrong path rather than passing vacuously. |
| Tests reading `assets/` via CWD | cargo sets CWD to the *package* root | build the path explicitly from `CARGO_MANIFEST_DIR`. |
| `include!(concat!(env!("OUT_DIR"), …))` | `OUT_DIR` is per-package | the generating `build.rs` must live in the same crate. |
| `tests/physical_design.rs` `ALLOWLIST` | path strings | update them; the stale-entry test catches this, so let it. |
| `pub(crate)` items used across the new boundary | crate-private no longer reaches | see below — think before widening. |

## Widening visibility is a design decision, not a chore

When a caller can't reach something any more, ask *why* it was reaching:

- **Ordering against a system by name** (`.after(some_fn)`) forces the
  owning crate to make the system *and its parameter types* public.
  Publish a `SystemSet` instead and keep the system private — see
  `dialogs::combobox::ComboboxEscapeSet` and
  `gameplay::plugin::MusicVolumeSet`.
- **Genuinely shared behaviour** (e.g. `pause_menu::apply_quit`) is fine to
  make `pub`.
- **Reaching through a re-export chain** (`audio_system::synth` when
  `synth` lives in core) means the call site should name the real owner.

## Verify

```bash
cargo build --features dev
cargo test --features dev     # count must not drop
cargo clippy --all-targets -- -D warnings
cargo check --lib --target wasm32-unknown-unknown
```

These cover every member because the workspace sets `default-members`.
A new crate under `crates/` is picked up by the `crates/*` glob
automatically — but if you add one *outside* that glob, add it to both
`members` and `default-members`, or its tests silently stop running.

Sum `test result:` lines including `FAILED` — counting only `ok` makes a
failure look like tests going missing.

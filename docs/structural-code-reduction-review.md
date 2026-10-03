# Structural code reduction review

This review examines opportunities beyond extracting small helpers. It does not implement the proposed architectural changes. The earlier reductions remain documented in `code-reduction-review.md`.

## Conclusion

There are worthwhile structural reductions, especially in editor persistence and UI controls. The evidence does **not** support cutting tens of thousands of lines while retaining all current behavior. A planning range for the candidates below is approximately **650–1,400 net production physical lines**, with the editor conversion work carrying most of the uncertainty. These are estimates, not demonstrated savings; replacement helpers and adapters are included conceptually, but only an implementation can establish the actual net reduction.

A further change to editor metadata ownership might remove another 100–250 lines. It needs a separate design decision about note versus onset ownership and is excluded from the initial total.

## Counting basis

The review scanned Rust files under `src/` and each crate's `src/`. Benchmarks and integration-test directories outside those roots are excluded. Inline test items and separate test modules were excluded using a lexical, indentation-based scan rather than a Rust AST, so the production split is approximate.

| Measure | Lines |
| --- | ---: |
| Physical Rust source lines in this scope | 97,719 |
| Approximate physical lines outside tests | 69,149 |
| Approximate nonblank, non-comment lines outside tests | 48,604 |
| Approximate test lines within this scope | 28,570 |

The larger count including Rust benches is 99,089. The user's approximate 75k baseline likely uses a different scope or counting convention; reductions should be compared using the same convention before and after.

The editor has approximately 14,694 physical non-test lines and gameplay 15,775. Together they account for about 44% of production source in the scanned scope. These are the best places to focus. Removing tests, documentation, blank lines, or moving Rust declarations into assets would change counts without necessarily simplifying the application.

## Ranked candidates

Ranges below count net physical production lines, including the cost of new shared code. The opportunities are separated to avoid counting the same editor conversion code twice.

| Candidate | Estimated net reduction | Confidence | Main risk |
| --- | ---: | --- | --- |
| Typed editor chart conversion instead of repeated JSON field construction | 300–650 | Medium-low | Changing saved chart semantics or losing extension fields |
| One shared slider track/readout implementation | 180–300 | Medium | Resource synchronization, localization, and different layouts |
| Central editor field access and metadata mapping | 70–160 | Medium-low | Invalid intermediate numeric input must remain editable |
| Common playable-note resolution and fewer enum adapters | 60–130 | Medium-low | Fractional bends and explicit pitches have different policies |
| Shared renderer note-state projection | 40–100 | Low | Abstraction cost can consume the savings |
| **Initial total, rounded** | **650–1,400** | | |

### 1. Editor chart conversion: the largest supported opportunity

Evidence: `crates/harmonicon-editor/src/song_editor/harpchart.rs`, especially `serialize_harpchart_notes` at line 57 and `load_harpchart` at line 394; `crates/harmonicon-core/src/chart.rs`; editor state and note model modules.

The serializer is about 286 physical lines and the loader about 261. Both mix document semantics with field-by-field JSON construction or lookup. The core already has typed chart structures and serde support. Keeping JSON access in both directions creates another place to maintain chart fields alongside the core schema and the editor state.

Proposed boundary:

1. Keep editable text buffers for the form.
2. Convert validated buffers and grid notes into a typed chart document through explicit adapters.
3. Serialize the document with serde; deserialize once into a typed document before populating the editor.
4. Preserve fields that the editable model cannot represent through an explicit extension/pass-through mechanism.

This can remove substantial field plumbing. It cannot remove the musical conversion algorithms. In particular:

- Serialization groups notes by both onset and duration; grouping only by onset would break unequal-length chords.
- Lyrics are emitted once per onset, even when several note groups share that onset.
- Tick/second conversion, pickup handling, tempo maps, repeats, and custom harmonica layouts still need explicit treatment.
- The editor stores `metadata.audio_file`, which is not currently represented by the core metadata model.
- `preserved_scoring` retains scoring details that the form does not edit. A typed-only rewrite must preserve that behavior.
- Unsupported feature validation must remain; successful deserialization does not imply the editor can safely edit every chart.

The 300–650 estimate spans the codec and adjacent conversion adapters. It assumes the new boundary genuinely replaces existing mapping code rather than layering another document model over it. Start with one direction and measure; stop if preserving all semantics requires more plumbing than it removes.

### 2. Sliders: the best first implementation

Evidence: `crates/harmonicon-menu/src/menu/pages/options/sliders.rs`, `options/zoom.rs`, and `crates/harmonicon-gameplay/src/gameplay/pause_menu.rs`.

Volume, latency, zoom, practice speed, and learned-threshold controls repeat track construction, range normalization, fill entities, readout entities, and visual synchronization. Options already shares row and label shells; the remaining opportunity is deeper than another row helper.

Add a small shared UI control containing the track, fill, and optional readout relationship. Share normalized fill updates and control construction. Keep resource mutation observers and value formatting with the feature that owns them. Accept dimensions and colors as ordinary parameters because options and pause controls differ.

Do not introduce a generic settings framework. A control should not need to know about audio resources, practice state, or persistence. Local systems should still decide when a resource change updates a control.

Validate range endpoints, stepped values, programmatic setting changes, reopened menus, and localization changes. This is relatively contained and should give a clear before/after count.

### 3. Editor field mapping

Evidence: `song_editor/state.rs` lines 488 and 531, `details_fields.rs`, `meta_form.rs`, and chart metadata conversion.

`field_text` and `field_text_mut` maintain parallel exhaustive matches. Field definitions, labels, defaults, cycle choices, validation, and persistence also refer to many of the same properties. The two accessor functions alone are about 84 physical lines; eliminating them alone is not a major saving.

A narrow field declaration mechanism could generate accessor mappings and associate parsing/display policies with each field. Alternatively, store form buffers independently from the typed document so field access has one representation and the validated document has another.

Prefer the smallest approach that removes several existing mappings. Avoid a universal string-keyed property bag or a large macro language. Text buffers are necessary: users must be able to type an empty value or a partial number without the application silently replacing it. Key changes must still invalidate the loaded custom harmonica as they do today.

This estimate excludes JSON field plumbing counted in candidate 1. Lesson manifest serialization is a smaller adjacent opportunity, but localized title/body representations require explicit conversion rather than blindly deriving serialization on the form.

### 4. Shared playable-note resolution

Evidence: `song_editor/playback.rs` line 110, `song_editor/harpchart.rs` line 27, editor `pitch_map.rs` and `note_model.rs`, and gameplay note/remapping modules.

Several paths translate direction and technique into the harmonica reed pitch, apply a bend, and convert the result to a frequency or MIDI note. Editor `Dir`/`Pitch` also overlap with core chart/pitch-map vocabulary, requiring small adapters.

A core resolver can return a precise pitch representation, with thin caller-specific policy for rounding, fallback, and unsupported notes. Sharing the vocabulary could remove some adapters as well.

Do not collapse different policies accidentally: playback retains fractional bends, MIDI conversion rounds them, explicit chart pitches bypass some harmonica lookup, and invalid-note display can use a fallback. Enum aliases alone would produce only a small reduction; the estimate depends on sharing actual resolution logic.

### 5. Renderer note-state projection

Evidence: `crates/harmonicon-gameplay/src/gameplay/gameplay_2d.rs` and `gameplay_3d/notes.rs`.

Both renderers project hold/judgment state into material uniforms and visual emphasis. A shared calculation can return the visual state, leaving each backend responsible for material lookup and geometry updates.

Much is already shared: scheduled note construction, note spawning decisions, pitch lists, feedback calculations, and HUD construction. Therefore the remaining gain is modest. A renderer trait or a generic ECS backend would likely cost more code than this saves.

The 3D judgment animation also scans label entities for each judged note. Keeping direct label entity references could avoid this repeated search. That is a useful algorithmic improvement, but it should be justified by behavior and performance rather than presented as a large LOC reduction.

## Additional candidate requiring an ownership decision

Editor intensity and annotation metadata live in side maps. `selected_metadata.rs` and `metadata_sync.rs` consequently maintain orphan cleanup, move/copy rekeying, and deletion behavior. Some operations scan notes repeatedly for selected IDs.

Putting note-specific intensity on `GridNote` and explicitly modeling onset-owned annotations could remove some synchronization code and repeated searches. Estimated net saving: 100–250 lines, with low confidence until the ownership model is specified.

Annotations cannot simply be copied onto every note: chords can have different durations, and annotations belong to an onset or phrase rather than necessarily to one member of a chord. Embedding additional fields also affects the current lightweight/copyable note representation. Preserve existing undo coverage instead of inadvertently extending or narrowing it during this change.

## Areas that do not support large savings

- **Undo:** snapshot code repeats a small set of fields, but most of the module implements actual history behavior. The repeated capture/restore portion is tens of lines, not hundreds.
- **Editor grid:** `rebuild_grid` is roughly 481 physical lines and `spawn_note` roughly 204. Their size comes largely from distinct visual elements and interactions. Splitting these functions would improve navigation but would not itself reduce code.
- **Scoring:** core scoring primitives are already shared. Editor practice and gameplay differ in pitch precision, chords, timing, technique bonuses, adaptive behavior, and wait modes. A single engine with many mode flags is not an evidenced simplification.
- **MIDI:** score import adapters are already shared. Editor MIDI import retains tempo/meter and backing information beyond the score abstraction. Removing the editor importer requires extending that abstraction, not merely deleting an apparent duplicate.
- **2D/3D rendering:** different geometry, materials, and disappearance rules are intentional. The shared HUD and feedback logic already remove the largest straightforward overlap.
- **Jam and scored call/response:** dynamically generated bars and authored phrase progression have different scheduling behavior. They already share synthesis primitives.
- **Clocks/metronomes:** helpers are shared, while independent session clocks are intentional. One global clock would change behavior.
- **Genre arrangements:** long backing arrangement functions are mostly musical data. Moving patterns to JSON shifts lines into assets and adds parsing/validation; it does not automatically reduce the system's complexity.
- **Crate merging:** dependency boundaries are already deliberate. Merging crates could remove a little module wiring but would not remove their implementations.

A normalized comparison of larger functions found only a few strong whole-function matches: single/multiline text input, two pause sliders, and 2D/3D note visual updates. This is supporting evidence, not a proof of uniqueness: structural duplication often survives despite different local function shapes.

## Implementation order and acceptance criteria

1. Implement the shared slider control and establish its actual net saving.
2. Prototype typed chart serialization with parity tests before replacing the loader. Check custom layouts, unequal-length chords, lyrics, tempo changes, pickups/repeats, audio metadata, and preserved scoring.
3. Reassess field mapping after the codec boundary is clear; do not create a competing model independently.
4. Consolidate pitch resolution where semantics match, then consider renderer projection only if the shared calculation remains small.
5. Treat metadata ownership as a separate design change.

For each change, count all new helpers and all removed call-site code across crates. Report production and regression-test deltas separately. Run existing affected tests and workspace checks; add tests for semantic boundaries rather than tests that merely mirror helpers. Reject an abstraction if its implementation and adapters consume the predicted savings or make behavior harder to follow.

This review changes documentation only. It makes no additional production-code changes and does not require a new test run.

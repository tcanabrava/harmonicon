# harmonicon-lessons

The lesson-facing feature crate: the skill tree, its pure layout engine, and
the lesson reader. Manifest loading, the prerequisite graph, unit gates, and
progress judgment remain in `harmonicon-song`.

The crate sits immediately above `harmonicon-menu` because lesson pages use its
`MenuPage` navigation state and shared page chrome. Keep that dependency narrow:
lesson-specific state and systems belong here; reusable widgets belong in
`harmonicon-ui`; lesson data and rules belong in `harmonicon-song`.

`lesson_tree/layout.rs` must remain renderer-independent. Geometry, ordering,
collapse inputs, and edge ports should be testable without spawning Bevy UI.
`lesson_tree/mod.rs` converts that output into entities and owns interaction and
animation. `lesson_reader.rs` owns lesson launch configuration and reader UI.

Update `docs/lesson_tree_layout_plan.md` as its phases land, pruning completed
steps rather than keeping an implementation diary.

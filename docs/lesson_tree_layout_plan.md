# Lesson tree layout and curriculum plan

**Implementation status: complete.** The extraction, collapsible major nodes,
downward dependency layout, continuous edge geometry, compacting canvas,
neighbor motion, viewport anchoring, accessibility state, and live-rescan
handling have shipped. The full workspace suite validates the bundled
curriculum graph and assets. The curriculum additions below remain the content
roadmap rather than prerequisites for the layout implementation.

This plan treats the lesson screen as a course dependency map: units are
course modules, lessons are classes, and prerequisite edges explain the order
in which skills become useful. The screen should answer three questions at a
glance: where am I, what can I study next, and why is something locked?

## Current layout findings

The findings below describe the renderer before the first implementation pass.
The downward course-map layout and single-segment edges shipped in `e58e620`;
they are retained here as the rationale for that change.

The pure layout engine in `harmonicon-lessons/src/lesson_tree/layout.rs`
already topologically layers lessons inside each unit, reduces crossings with
barycentre sweeps, and hides cross-unit edges behind the unit spine. The
remaining visual problems come from the rendering contract around that layout.

- Every edge is forced to leave and arrive horizontally. That suits
  lesson-to-lesson edges in the current left-to-right DAG, but unit-to-root
  edges connect a unit above to roots below. They leave the unit's right side,
  turn sharply downward, then enter the lesson from the left, producing the
  conspicuous hook-shaped curve.
- `spawn_edge` clips every endpoint using `NODE_PX / 2`, including the larger
  `UNIT_PX` unit nodes. Spine and branch endpoints therefore do not meet the
  visible boundary consistently.
- A curve is rendered as many independently rotated UI rectangles. The code
  extends them to overlap, but it does not attach the rounded caps described in
  its comment. Rotation, pixel rounding, and alpha blending at every overlap
  make the line look choppy. More samples create more blended joins.
- A unit is anchored to its cluster's first column rather than centered over
  the bounds of its children. Wide or asymmetric clusters read as loosely
  associated groups.
- The whole curriculum is always expanded, so horizontal size grows with every
  lesson and makes the course harder to scan as it becomes more complete.

## Target interaction and geometry

### 1. Collapsible units

**Initial interaction shipped in `0be52cc`; compact layout is now implemented.**
Unit nodes are focusable buttons whose accessibility state reports whether
their cluster is expanded, explicit choices persist across lesson-reader
visits, the chevron shows the current state, and all lesson art and local edges
animate over 220 ms. Once a close animation finishes, the pure layout reclaims
that cluster's columns and recomputes the canvas. The toggled unit keeps its
horizontal screen position across that rebuild, clamped at the new scroll
bounds. Lesson buttons leave pointer and keyboard interaction as soon as their
unit starts closing. Neighboring units slide to their recomputed columns over
220 ms, with each straight edge recalculated from its moving endpoints.
Reduced-motion behavior remains open until that application setting exists.

Make every unit node an accessible button with expanded/collapsed state. Keep
the current unit, the first available unit, and units containing available
lessons expanded on first visit; persist explicit choices for the session. A
collapsed unit stays on the spine and shows its title, lock state, completion
fraction, and a chevron. Its lesson nodes and local edges disappear.

Animate a per-unit `expansion` value from 0 to 1 over 180–240 ms with an
ease-out curve. Use it to interpolate cluster opacity and vertical scale from
the unit's lower anchor while neighboring units slide horizontally. Disable
lesson hit testing as collapse starts and restore it once expansion is
readable. A future reduced-motion setting should jump to the final state.

Collapse must change the layout input instead of only applying
`Display::None`: recompute canvas width, scroll bounds, spine positions,
keyboard tab order, and edge routes from visible clusters. Preserve context by
keeping the toggled unit's screen-space center fixed while neighbors move.

### 2. Course-map layout

**Core layout shipped in `e58e620`.** Prerequisite depth now increases
downward, siblings spread horizontally, and each unit is centered over its
widest lesson row. Deterministic tests cover direction, centering, spacing,
crossing reduction, and canvas bounds. Explicit named ports are no longer
needed by the current straight-edge renderer; endpoint kinds carry the node
sizes it needs.

Give units and lessons distinct layout roles. Lay the unit spine left to right.
For each expanded unit, compute its lesson DAG independently, measure its
bounding box, and center the unit over it. Put root lessons in a row below the
unit and increase prerequisite depth downward, like a university course map.
Order siblings within a depth row with the existing barycentre heuristic.

Use explicit ports in the layout result:

- spine edge: right port of one unit to left port of the next;
- unit edge: bottom port of the unit to top port of each root lesson;
- lesson edge: bottom port of a prerequisite to top port of its dependent;
- future cross-unit exception: a labeled boundary port instead of a line
  crossing other clusters.

The layout result should carry node bounds and edge ports, rather than making
the renderer infer them from `NODE_PX`. Reserve clearance for labels and
mastery pips, then route orthogonal or gently rounded paths through the gaps.
Add deterministic tests for cluster centering, endpoint sides, no node/label
intersections, stable ordering, collapsed widths, and canvas bounds.

### 3. Continuous edge rendering

**Resolved in `e58e620` with a simpler renderer.** Each relationship is one
straight, rotated UI rectangle with rounded ends. It is clipped along the line
between centers using the true radius of each endpoint; `UnitBranch` records
the mixed unit-to-lesson case. This removes every sampled join and its stacked
alpha while keeping UI scrolling and clipping. Keep the mesh approach below as
a fallback only if visual testing shows straight relationships need curves.

Replace the chain of UI rectangles with one continuous geometry path per edge.
The preferred implementation is a small UI mesh builder that emits a triangle
strip with bevel joins and round caps from an adaptive polyline. It preserves
UI clipping and scrolling while eliminating alpha-stacked joints. Tessellate
by curvature and screen-space error, and snap straight sections to physical
pixels.

Keep one edge component containing endpoints and style. Rebuild its mesh only
when layout, DPI scale, or expansion changes. During collapse, trim the path at
animated ports and fade it with the cluster. Add a visual debug mode for node
bounds, ports, control points, and tessellation segments.

## Delivery sequence

1. **Done:** flow the local DAG downward, center clusters, distinguish edge
   endpoint kinds, and test the geometry invariants.
2. **Done:** add unit-button state, keyboard focus, session persistence, and
   animated visibility for each cluster.
3. **Done:** collapsed clusters compact after their close animation,
   including safe rapid toggles, recomputed canvas bounds, and viewport
   anchoring. Live lesson rescans also remove transition state for units that
   disappeared. Neighboring units and their edge endpoints animate into the
   new layout. Add reduced-motion behavior once that setting exists.
4. **Done:** replace sampled curves with continuous straight paths. Each edge
   is one UI rectangle, so there are no tessellation joins whose appearance
   changes with scale. Keep multi-scale captures and a mesh renderer as
   regression tools only if a curved route is introduced later.
5. **Not activated:** add an overview/minimap or “show available” action only
   if usability tests show players losing their place. Viewport anchoring and
   compact units address the original navigation problem without another
   control, and there is currently no evidence that one is needed.

Reduced-motion behavior remains a shared-settings follow-up. The repository
does not currently expose such a preference; when it does, both cluster and
neighbor transitions should use it rather than introducing a lesson-only
setting.

## Curriculum gaps

The current 41 lessons form a strong blues-oriented beginner course and a
small jazz/chromatic bridge. New content should deepen playable technique
before adding more theory. Every item needs the honest scoring classification
required by `docs/lessons_plan.md`.

### Highest priority


### Technique expansion

- **Vibrato families and dynamics:** distinguish hand, throat, and diaphragm
  approaches instructionally; score outcomes such as oscillation rate/depth,
  crescendos, diminuendos, and long-tone stability.
- **Accompaniment:** chord rhythm, train accompaniment behind a singer, fills
  between vocal phrases, and trading fours. Reuse form and rest discipline.

### Chromatic and musicianship branches

- Grow chromatic harmonica beyond one slide-scale lesson: button coordination,
  enharmonic slide choices, chromatic fragments, legato slide phrasing, and an
  original or verified-public-domain jazz study.
- Add intervals, chord construction, guide tones, arpeggio-to-scale connection,
  transposition, and song-form listening as short theory-plus-play units.
- Teach practice skills: slow practice, loop isolation, spaced review,
  self-recording, and mixed review lessons that sample earlier dependencies.
  Guide use of existing tools instead of pretending the microphone can judge
  posture or practice quality.

Before authoring these, split the graph into a required core and optional
branches. Advanced techniques and repertoire should add depth without becoming
gates in front of rhythm, blues, or basic improvisation.

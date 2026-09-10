# Lesson tree layout and curriculum plan

This plan treats the lesson screen as a course dependency map: units are
course modules, lessons are classes, and prerequisite edges explain the order
in which skills become useful. The screen should answer three questions at a
glance: where am I, what can I study next, and why is something locked?

## Current layout findings

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

1. Extend the pure layout model with node bounds, cluster bounds, ports, and a
   set of expanded unit ids. Change the local DAG to flow downward and test the
   geometry invariants.
2. Add unit-button state, keyboard/focus behavior, session persistence, and an
   immediate collapse path. Verify scroll extents and focus order first.
3. Add expansion animation and viewport anchoring. Validate compact windows,
   DPI scaling, rapid toggles, and lesson rescans during animation.
4. Replace segmented edges with cached mesh paths. Compare captures at 1×,
   1.25×, 1.5×, and 2× scale, then remove the rectangle renderer.
5. Add an overview/minimap or “show available” action only if usability tests
   still show players losing their place after collapse ships.

## Curriculum gaps

The current 41 lessons form a strong blues-oriented beginner course and a
small jazz/chromatic bridge. New content should deepen playable technique
before adding more theory. Every item needs the honest scoring classification
required by `docs/lessons_plan.md`.

### Highest priority

- **Hole navigation and register changes:** landmarks, low/middle/high-register
  jumps, and patterns crossing holes 3–4 and 6–7. Use pitch/timing accuracy.
- **High-register blow bends:** controlled 8/9/10 blow bends followed by short
  melodic applications. Existing bend scoring should work after validating
  the relevant harmonica maps.
- **Rhythm progression:** eighth-note triplets, straight versus shuffle,
  syncopation, rests, sixteenth-note articulation, and tempo stability. Reuse
  timing windows, tempo maps, and fresh-attack scoring.
- **Ear training:** match a heard note, repeat a hidden short phrase, identify
  up/down/same motion, and find a tonic or chord root. Start from the existing
  call-and-response path and progressively remove visual hints.
- **Applied positions:** playable first-, second-, and third-position modules
  with register maps, tonic/chord-tone landing drills, and position-specific
  licks. The circle lesson explains six positions while playable work mainly
  cycles through three.

### Technique expansion

- **Tongue-block articulation:** slap, pull, side-pull, rake, flutter, and
  moving octaves after tongue blocking and octave splits. Most are
  instructional or scored through chord, pitch, and rhythm outcomes.
- **Warbles, shakes, trills, and glissandi:** move from slow adjacent-hole
  alternation to ornaments. A reusable alternation detector could honestly
  score event rate and pitch pattern later.
- **Vibrato families and dynamics:** distinguish hand, throat, and diaphragm
  approaches instructionally; score outcomes such as oscillation rate/depth,
  crescendos, diminuendos, and long-tone stability.
- **Overblows and overdraws:** the engine and bend trainer recognize them, but
  the curriculum never teaches them. Put them in an optional advanced diatonic
  branch so they do not block the core course.
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

# B4. Auto-routed running stitches, Redwork and satin branching: algorithm sketch

Backlog item B4 of [competitive-features.md](../competitive-features.md) (features D1, D2,
O10). Research date: 2026-10-28. A design note for `openstitch-design` (`route.rs`, a new
`graph.rs`) and `openstitch-engine` (`order.rs`): the graph model, the routing algorithms,
parameters, model change and tests. No other product's code was read; the graph algorithms
are textbook (Euler paths, route inspection, Hierholzer).

## 1. What the feature is

Three related outputs from one idea, "treat the drawing as a graph and walk it":

- **Auto-routed running stitches.** A set of open and closed running-stitch lines (a line
  drawing: a face, a map, lettering outlines) is reordered and connected so the needle sews
  all of it with as few jumps and trims as possible, travelling along lines already sewn
  (or about to be sewn twice) instead of jumping. Lines are split where they cross so the
  needle can turn at intersections.
- **Redwork.** The same, with the rule that every line is sewn **exactly twice** and the
  path is a closed loop that starts and ends at one point: no jumps at all, one trim, and
  the double pass reads as a bean stitch. The classic single-colour "redwork" look.
- **Satin branching.** Several satin columns that meet (the strokes of a letter, the veins of
  a leaf) are digitized as one object with one entry and one exit: the engine sews a
  centre-run underlay outward along the branches and the satin on the way back, so each
  branch is covered after its children, with no trim between the branches.

Who has it: Wilcom Branching, Bernina V9 Branching (headline feature)
[[ES-designing]](https://wilcom.com/embroiderystudio/designing),
[[Bernina-connectors]](https://softwarehelp.bernina.com/en/MainHelp/PART_Digitizing/connectors/connectors-1.htm);
Pulse Branching with Automatic Overlap / Fix Gap at branch ends
[[DG16-IE]](https://tajimaeurope.com/en/pulse-software-solutions/embroidery-software-tajima-software-dg16-pulse/illustrator-extreme/);
Embrilliance object branching [[EM-SA]](https://www.embrilliance.com/stitchartist); Ink/Stitch
Auto-Route Running Stitch (add nodes at intersections, trim jump stitches, preserve order),
Auto-Route Satin Columns, Redwork (connect lines below a distance, minimum path length, bean
repeats on the top layer) [[IS-stroke]](https://inkstitch.org/docs/stroke-tools/),
[[IS-satintools]](https://inkstitch.org/docs/satin-tools/); Wilcom ES 2026 Redwork
[[ES-designing]](https://wilcom.com/embroiderystudio/designing); the routing idea discussed
on the Inkscape developer list (duplicate edges so the graph has an Euler path)
[[inkscape-devel]](https://lists.inkscape.org/hyperkitty/list/inkscape-devel@lists.inkscape.org/thread/C7XJD67MCZNGO2WJOARMERURZ7GQYTXE/).

PE-DESIGN 11: Optimize Sewing Order (colour grouping, OpenStitch `order.optimize` at the
reference tier), Optimize Entry/Exit Points (`order.linkEntryExit`), Connect Points
(joining two outline paths into one). No routing through intersections, no redwork, no
branching. My Design Center on Brother machines makes line drawings that would benefit most.

## 2. The graph

Input: the selected line objects (running, triple, or any line type whose stitches are a
single pass along the path; motif and E/V lines are excluded from routing but can be
sequenced). Build an undirected multigraph `G`:

1. **Vertices** at every path end and at every crossing or touching point between two paths
   (or a path and itself), found by segment intersection over the flattened paths with a
   tolerance `join` (default `0.5 mm`): endpoints within `join` of another path or of each
   other are snapped to one vertex, so a drawing whose strokes nearly meet is treated as
   connected. Ink/Stitch calls the first part "add nodes at intersections" and the second
   "connect lines below this distance"; both are needed for hand-drawn or traced art.
2. **Edges**: the path pieces between consecutive vertices, each with its geometry (the
   piece of the flattened path), its length as weight, and its source object and line
   attributes.
3. **Components**: connected components of `G` are routed separately and then sequenced as
   objects (nearest component next, by the closest pair of vertices).

Closed paths with no intersection are a single vertex with a loop edge; they are routed as
one edge.

## 3. Auto-route running stitches (Euler path with minimal duplication)

For a component, a walk that uses every edge exactly once exists when zero or two vertices
have odd degree (Euler circuit or path). Otherwise some edges must be sewn twice. The
cheapest doubling is the **route inspection** (Chinese postman) solution:

1. Odd-degree vertices `O` (always an even count).
2. All-pairs shortest paths between vertices of `O` (Dijkstra from each; components are
   small, at most a few hundred vertices in a line drawing).
3. Minimum-weight perfect matching on `O` with those distances. For `|O| ≤ 12` solve exactly
   (bitmask dynamic programming, `3^(|O|/2)`-ish, instant); above that use the greedy
   nearest-pair matching followed by 2-opt improvement, which is what the output quality
   needs (the exact blossom algorithm is not worth a dependency here).
4. Duplicate the edges along each matched shortest path. Every vertex is now even.
5. Choose the start: the vertex nearest the previous object's exit (or the design's start
   position); with an Euler path (two odd vertices before doubling) the start must be one of
   them, so choose the nearer; the end is then forced.
6. **Hierholzer's algorithm** produces the circuit. At each vertex, when several unused edges
   are available, prefer (a) the edge that continues straightest (smallest turn), then (b)
   an edge not yet sewn over a duplicated one, so duplicated passes land late and over
   already-sewn thread. These tie-break rules change nothing in the Euler property and are
   what makes the result look digitized by hand.
7. Emit: a single run per component in walk order; duplicated edges are sewn as the same
   line type (a running line sewn twice is the "travel"); the component's run gets the line
   attributes of its first edge, and a colour change is inserted when consecutive edges have
   different threads (so a multi-colour drawing becomes one run per colour group, with the
   matching done per colour).

Cost model and options:

- `trimJumps` (default on): a connector between components longer than `jumpMin` (the
  design's jump-trim length, as for every object) becomes a trim; shorter ones are a jump.
- `preserveOrder` (default off): keep the objects' order and only connect them (no
  reordering); the walk is then per object with travel back along the object.
- `maxDouble` (default `∞`): a matched path longer than this is replaced by a jump; useful
  on sparse drawings where a long travel run would be visible.

The doubled edges are the only "cost" of the method; for a typical line drawing they are
10–25 % of the total length, which is less than the thread a jump-plus-trim consumes, and
they are hidden because the second pass lies on the first.

## 4. Redwork

Double **every** edge up front (`G'` = each edge twice). Every vertex is now even, so an
Euler circuit exists with no matching at all; Hierholzer from the start vertex gives a closed
walk that sews each line exactly twice and returns to the start. Tie-breaks as above, plus:
prefer the unused twin of the edge just sewn when turning back is required, so the second
pass of a dead-end branch follows the first pass immediately (a leaf is sewn out and back).
Options:

- `repeats` for the top pass (default 1): each edge's second traversal is sewn as a bean
  stitch of that many extra passes so the double pass reads bold; the first traversal is
  plain.
- `minPath` (default `1 mm`): edges shorter than this are merged into their neighbours
  before routing.
- `join` as in section 2.

One run, one colour, one trim at the end. Components are chained by the nearest vertices; a
connector between two components is a jump (and trim) because redwork drawings are usually
one piece.

## 5. Satin branching

Input: several satin columns (Manual Punch columns, zigzag lines, skeleton satins) whose
centrelines meet, selected together, plus optional entry and exit points on the group.

1. Build `G` on the **centrelines** (rung midpoint chains) with the join tolerance scaled to
   the column width (`join = max(0.5 mm, w/2)`), so two columns whose ends overlap are one
   vertex. Each edge carries its rungs.
2. Choose the root vertex: the entry point's nearest vertex (default: the vertex nearest the
   previous object's exit).
3. **Spanning tree** of the component from the root (the drawing's own structure; a cycle in
   the graph, like the loop of a "P", is broken at the edge farthest from the root, which is
   then sewn as a branch with a short overlap at the break).
4. **Depth-first walk** from the root. Going **down** an edge sews its centre-run underlay
   (a running stitch along the centreline, `2.5 mm` pitch, what `UnderlayStyle::Edge` for
   satin is today in essence); at a leaf the walk turns back and, going **up** an edge, sews
   its satin from the far end towards the root. So every column's satin is sewn after all of
   its children's satin and after its own underlay, and the needle never leaves the object:
   one run, one entry, one exit, no trims. The order among children at a vertex: nearest
   first by the angle to the incoming direction (sew the side branches before continuing
   straight), which keeps the final "trunk" satin on top.
5. **Junction overlap**: at a vertex, each child's satin is extended `overlap` (default
   `w/2`, Pulse's Fix Gap) into the parent column, so the parent's satin covers the joint
   with no gap; corners of the extended end are square (B3 `cap` logic is not applied at
   junctions).
6. Exit: the walk ends at the root; if an exit point is set elsewhere, the final column is
   chosen as the branch containing the exit (the tree is re-rooted so the walk ends there)
   and the entry is the other end, the usual two-ended object.
7. Emit one object of kind `SatinBranch` (or a group of the columns with a `branch: true`
   flag) so the editor keeps the columns editable; the compiler regenerates the walk from
   the columns on every change.

Underlay beyond the centre run (edge run, zigzag) is per column as today and sewn on the way
down as well. Short stitches and corners (B3) apply per column.

## 6. Parameters and defaults

| Command / attribute | Type | Default | Notes |
|---|---|---|---|
| `order.autoRoute` | command on selected lines | — | options below |
| `join` | mm | 0.5 | snap tolerance for endpoints and crossings |
| `trimJumps` | bool | true | connectors longer than the jump-trim length become trims |
| `preserveOrder` | bool | false | connect without reordering |
| `maxDouble` | mm | none | matched travel longer than this becomes a jump |
| `order.redwork` | command on selected lines | — | |
| `repeats` | 1–3 | 1 | bean passes on the second traversal |
| `minPath` | mm | 1.0 | merge shorter edges |
| `object.branch` | command on selected satins | — | makes one branched object |
| `overlap` | mm | w/2 | junction extension |
| `underlayPitch` | mm | 2.5 | centre-run pitch on the way down |

`order.autoRoute` and `order.redwork` are edits that replace the selected line objects by the
routed lines (one `shape.path` per run, with the pieces' attributes), undoable in one step;
the originals can be kept with `keep: true` for comparison.

## 7. Model and code changes

- `openstitch-design/src/graph.rs` (new): `StitchGraph` built from flattened paths
  (intersections via the segment sweep already used by `boolean`/`i_overlay` or a simple
  O(n²) over pieces with a bounding-box prefilter, fine for line drawings), `euler_walk`
  (route inspection + Hierholzer with the tie-breaks), `double_all`, `spanning_tree`,
  `dfs_order`. Pure, deterministic, with the point budget.
- `route.rs`: `travel` already walks an outline between two points; reuse for connectors
  along existing paths.
- `model.rs`: `ObjectKind::SatinBranch { columns: Vec<...> }` or a `branch: bool` on groups;
  proposal: a group attribute, so grouping and ungrouping stays the editing model and the
  compiler checks the flag.
- `compile.rs`: when a group has `branch`, compile its satin members through `dfs_order`.
- Engine: commands `order.autoRoute`, `order.redwork`, `object.branch` / `object.unbranch`;
  capabilities `edit`.
- App: Home ▸ Sewing Order gets "Auto route" and "Redwork"; the Manual Punch/Outline tabs get
  "Branch" for a multi-selection of satins; the Sewing Order pane shows a branched group as
  one item with its columns indented.

Effort: M for the running-stitch router and redwork (graph build, matching, walk, commands,
tests), L for branching (centreline graph on rung chains, junction overlaps, the compile
path, editor state) as the main document estimates.

## 8. Test cases (internal regression tier)

| Spec | Script | Expected |
|---|---|---|
| `route-euler-path` | three open running lines forming a "Y" (two odd vertices), `order.autoRoute` | one run; every edge sewn once (total length = sum of edges ± pitch); zero jumps inside the component |
| `route-matching-square-x` | a square with both diagonals (four odd vertices), auto route | one run; doubled length ≤ the two shortest matching paths (two sides); zero jumps; no edge sewn more than twice |
| `route-two-components` | two separate drawings 30 mm apart | two runs joined by one jump that is a trim (`trimJumps`); the second component starts at its vertex nearest the first's end |
| `route-snap-join` | two lines whose ends are 0.3 mm apart, `join` 0.5 | treated as one vertex: one run, zero jumps; with `join` 0.1 they are two components |
| `route-crossing-adds-node` | two lines crossing once | the walk turns at the crossing (four edges, one run, no jump), not two separate passes |
| `route-preserve-order` | the square-with-diagonals in a given object order, `preserveOrder` | object order kept; connectors are travel along already-sewn edges, zero jumps |
| `route-colours` | the "Y" with one arm in another colour | two runs, one colour change, each run routed within its colour |
| `redwork-tree` | a tree drawing (trunk, three branches, no cycle), `order.redwork` | one run, closed (first and last stitch within 0.5 mm); every edge traversed exactly twice (coverage count per edge = 2); zero jumps |
| `redwork-cycle` | a circle with two spokes | same properties; the circle is traversed twice too |
| `redwork-repeats-3` | the tree with `repeats` 3 | second-traversal stitches tripled: stitch count ≈ edges × (1 + 3) passes ± 5 % |
| `branch-letter-T` | two 6 mm Manual Punch columns meeting as a T, `object.branch` | one run; the crossbar's satin is sewn after the stem's (stem = child, crossbar = root containing the entry); underlay runs precede every column's satin; zero jumps and trims; the stem's satin extends `w/2` into the crossbar (needle points inside the crossbar's outline near the joint) |
| `branch-leaf-veins` | a midrib with four veins | one run; every vein's satin sewn before the midrib's; children ordered nearest-angle first |
| `branch-cycle-P` | a "P" skeleton (stem plus loop) | the loop is broken once; one run; overlap at the break; zero trims |
| `branch-exit-point` | the T with an exit at the crossbar's far end | the walk ends within 0.5 mm of the exit; the entry is the other end |
| `branch-ungroup-regression` | the T columns compiled without `branch` | identical to today's two-object output |

Measurement helpers needed: "edge coverage count" (map each needle point to the nearest
input edge and count passes), "jump and trim counts" (exist), "run count" (exists), "needle
points inside a polygon" (shared with B1–B3).

## 9. Compatibility and licensing

New commands and a group flag; nothing existing changes. The reference spec
`optimize-entry-exit` keeps testing `order.optimize` and `order.linkEntryExit` as today.

Euler paths, route inspection and Hierholzer's algorithm are nineteenth- and twentieth-
century graph theory; the matching is solved exactly for small sets and heuristically
otherwise. Ink/Stitch's documentation gave the option names (add nodes at intersections,
trim jumps, preserve order, connect lines below a distance, minimum path length); its
implementation was not read. "Branching" is a generic term used by every vendor.

## 10. Open questions

- Whether `order.autoRoute` should also accept satin lines (zigzag lines) as edges: they
  are one pass too, so the router works, but the doubled passes would be visible; proposal:
  allow, with travel passes sewn as running stitch under the satin (which is what the
  branching walk does).
- The exact matching cut-over (12 odd vertices) and whether to add a small blossom
  implementation later if drawings with many loose ends look worse under the heuristic.
- Whether branching should be an `ObjectKind` of its own (one object, columns inside) or a
  group flag; the flag keeps every existing editing command working on the columns, which
  is why it is proposed.

# P8-M2-T03: Where-used + impact query on the relationship graph

Status: implemented as part of task P8-M2-T03 (isolith card t_7b63c5f9).
Owner: open-engineering-platform P8 PDM lane.

## Relationship graph model

The PDM relationship graph is a directed graph over vault items:

- Node = a vault item (part, assembly or document), identified by its item id
  (the same opaque id the v1 API contract uses for `Item`).
- Edge = a `composed_of` (BOM / uses) relationship from a parent item to a
  child item, with a quantity = number of distinct placements of the child
  inside the parent. The graph records outgoing (parent -> child) and
  incoming (child -> parent, i.e. "used by") adjacency, so both directions
  are O(1) to traverse.

The graph is deliberately structural: it models *what uses what*. Revision
state is carried as optional per-item metadata (Working / UnderReview /
Released / Obsolete) so impact queries can distinguish "affects released
assemblies" from "affects work in progress". The revision state machine
itself is P8-M2-T01's card; here revision state is only *consumed* as a flag.

## Queries

### where_used(item)

Returns where an item is used across assemblies:

- `direct` uses: assemblies that compose the item directly (one hop), each
  with the placement quantity and the path (the assembly itself).
- `transitive` uses: assemblies that reach the item through nested
  sub-assemblies (two or more hops), each with the shortest path from the
  reached assembly down to the direct parent and the hop depth.

Both lists are deduplicated per reached assembly (min-depth BFS wins).
Unknown items and empty graphs return an empty report, never an error - the
tolerant-failure doctrine (P2-M1-T04) applies to queries.

### impact(change)

Takes a revision change `{ item, from_rev, to_rev }` and returns the blast
radius of that change over the relationship graph:

- `affected`: every assembly that directly or transitively composes the
  changed item, with the path, hop depth, direct quantity, the assembly's
  current release state (if recorded) and a `released` flag.
- `affected_total` / `affected_released`: summary counts; a released count
  > 0 is exactly the "what a revision change affects" answer for a released
  product line - every listed released assembly must be revalidated /
  re-released because it pins the changed item.

Affected assemblies are computed as the where-used closure of the changed
item, so the impact of a leaf-part change is reported all the way up to
top-level released assemblies.

## Real-material integration

The graph ingests the real oep-geo assembly documents that the P7 assembly
workbench (P7-M3-T01) produces and validates:

- `crates/oep-assem/tests/golden/two-part-fixture.oepgeo.json`
- `crates/oep-geo/tests/golden/example-bracket.oepgeo.json` (a real
  sub-assembly structure: bracket assembly -> fastener-set sub-assembly ->
  four real fasteners)

`RelationshipGraph::from_oepgeo` maps every part to a part node, every
assembly to an assembly node, and every instance to a composed_of edge
(parent assembly -> source part/sub-assembly), aggregating quantity per
distinct source id. Then the where-used and impact queries run unchanged on
that real structure - this is the integration test on P7 assembly workbench
output the acceptance requires.

## Performance budget

A "sized fixture" for this card is a deterministic synthetic BOM: top-level
assemblies with nested sub-assemblies over a large shared part pool
(20k+ edges, ~6k nodes). The where-used and impact queries are both O(V + E)
per query on the reverse adjacency (BFS with a visited set, no recursion, so
deep/narrow BOMs cannot overflow the stack). Acceptance: a single query on
the sized fixture resolves well under the CI-safe budget on the reference
host (U1), verified by a performance test that measures and enforces a bound
with a documented, measured baseline.

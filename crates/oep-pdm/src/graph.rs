//! P8-M2-T03: relationship-graph queries -- where-used and impact.
//!
//! This module owns the PDM *relationship graph*: the directed "uses /
//! composed-of" structure over vault items, plus the two query surfaces the
//! card's acceptance mandates:
//!
//! * [`RelationshipGraph::where_used`] -- given an item, where is it used
//!   across assemblies (direct placements and nested sub-assembly parents),
//!   with per-assembly quantity and the containment path.
//! * [`RelationshipGraph::impact`] -- given a revision change to an item,
//!   what does it affect: every assembly in the used-by closure, with its
//!   path, quantity, and lifecycle state (released assemblies are the ones
//!   that must actually be revalidated / re-released).
//!
//! Both queries are iterative BFS over adjacency maps (no recursion), so a
//! deep or narrow BOM cannot overflow the stack, and each query is
//! O(V + E) in the size of the reachable component. Unknown items and empty
//! graphs return an empty report, never an error (tolerant-failure
//! doctrine, P2-M1-T04).
//!
//! The graph also ingests the real oep-geo assembly documents the P7
//! assembly workbench produces ([`RelationshipGraph::from_oepgeo_document`]),
//! which is what the card's integration acceptance runs on.
//!
//! The revision state machine itself (working / under_review / released /
//! obsolete) is P8-M2-T01's card; here lifecycle state is only *consumed* as
//! a flag on impact results.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};

use oep_geo::Document;

/// Kind of a vault item in the relationship graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    /// A manufactured part.
    Part,
    /// An assembly (which composes parts and/or sub-assemblies).
    Assembly,
    /// A document (e.g. a drawing or spec) that can reference items.
    Document,
}

/// Lifecycle state of an item revision, used by impact to flag released
/// assemblies (the ones a change actually forces to revalidate).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum LifecycleState {
    /// A draft revision; changes are expected, impact is informational.
    Working,
    /// In review; not yet released but candidates for it.
    UnderReview,
    /// Released; a change to anything it uses forces revalidation.
    Released,
    /// Superseded; no longer a live consumer.
    Obsolete,
}

/// A reference edge in the relationship graph: `parent` composes `child`
/// with a placement quantity (how many instances of the child appear in the
/// parent assembly).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsageEdge {
    /// Item id of the parent (the assembly that uses the child).
    pub parent: String,
    /// Item id of the child (the used part or sub-assembly).
    pub child: String,
    /// Number of instances of `child` placed in `parent` (>= 1).
    pub qty: u32,
    /// Optional pinned child revision this usage refers to; `None` means
    /// "any revision" (no revision pinning recorded on this edge).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub child_revision: Option<String>,
}

impl UsageEdge {
    /// Convenience constructor for an unpinned usage edge.
    pub fn new(parent: impl Into<String>, child: impl Into<String>, qty: u32) -> Self {
        UsageEdge {
            parent: parent.into(),
            child: child.into(),
            qty,
            child_revision: None,
        }
    }
}

/// Static metadata for an item node in the graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemMeta {
    /// Item id (matches the v1 API contract's `Item.id`).
    pub id: String,
    /// Human item name.
    pub name: String,
    /// Item kind.
    pub kind: ItemKind,
}

/// One recorded revision of an item, with its lifecycle state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevisionRecord {
    /// Revision identifier, e.g. `"1"` or `"C"`.
    pub revision: String,
    /// Lifecycle state of this revision.
    pub state: LifecycleState,
}

/// Single error type for graph construction/ingest failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphError {
    /// A required referent is missing (e.g. an instance references a part
    /// that is not present in the source document).
    DanglingReference {
        /// The referring assembly item id.
        parent: String,
        /// The missing referenced id.
        missing: String,
    },
    /// An item id was given without a required non-empty name.
    MissingName(String),
    /// A malformed document header / format mismatch.
    InvalidDocument(String),
}

impl std::fmt::Display for GraphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GraphError::DanglingReference { parent, missing } => {
                write!(f, "assembly '{parent}' references missing item '{missing}'")
            }
            GraphError::MissingName(id) => write!(f, "item '{id}' registered without a name"),
            GraphError::InvalidDocument(m) => write!(f, "invalid oep-geo document: {m}"),
        }
    }
}

/// One hit in a [`WhereUsedReport`] or [`ImpactReport`].
///
/// `path` lists the assembly chain from the reported assembly down to the
/// direct parent of the target item, e.g. `["top-assembly", ...,
/// "sub-assembly"]`; `depth` is the number of hops from the target item
/// (1 = the direct parent). `qty` is the product of the edge quantities
/// along that path (how many of the target item the reported assembly
/// contains).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsageHit {
    /// Item id of the assembly that (directly or transitively) uses the
    /// target item.
    pub assembly: String,
    /// Human name of that assembly.
    pub assembly_name: String,
    /// Containment path: from this assembly down to the direct parent.
    pub path: Vec<String>,
    /// Number of hops from the target item (1 = direct parent).
    pub depth: usize,
    /// Product of edge quantities along `path`.
    pub qty: u32,
    /// True when a direct placement (this assembly instantiates the item
    /// directly, depth == 1).
    pub direct: bool,
}

/// Result of [`RelationshipGraph::where_used`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WhereUsedReport {
    /// The queried item id.
    pub item: String,
    /// Direct parents (depth 1).
    pub direct: Vec<UsageHit>,
    /// Arrive via nested sub-assemblies (depth >= 2).
    pub transitive: Vec<UsageHit>,
}

impl WhereUsedReport {
    /// All hits, direct and transitive (direct first, then by depth).
    pub fn all(&self) -> impl Iterator<Item = &UsageHit> {
        self.direct.iter().chain(self.transitive.iter())
    }
}

/// A revision change to an item: `from_revision` -> `to_revision`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Change {
    /// Item id whose revision changed.
    pub item: String,
    /// Revision the item is changing from.
    pub from_revision: String,
    /// Revision the item is changing to.
    pub to_revision: String,
}

impl Change {
    /// Convenience constructor.
    pub fn new(
        item: impl Into<String>,
        from_revision: impl Into<String>,
        to_revision: impl Into<String>,
    ) -> Self {
        Change {
            item: item.into(),
            from_revision: from_revision.into(),
            to_revision: to_revision.into(),
        }
    }
}

/// One affected assembly entry in an [`ImpactReport`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Affected {
    /// Assembly item id.
    pub assembly: String,
    /// Assembly name.
    pub assembly_name: String,
    /// Containment path from this assembly down to the direct parent.
    pub path: Vec<String>,
    /// Hops from the changed item (1 = direct consumer).
    pub depth: usize,
    /// Quantity of the changed item inside this assembly.
    pub qty: u32,
    /// True when this assembly has at least one Released revision
    /// (must be revalidated if the change is promoted).
    pub released: bool,
    /// Released revision ids of this assembly (empty when not released).
    pub released_revisions: Vec<String>,
    /// True when the assembly instantiates the changed item directly.
    pub direct: bool,
}

/// Result of [`RelationshipGraph::impact`]: every assembly a revision change
/// affects, through the full used-by closure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImpactReport {
    /// The change that was applied.
    pub change: Change,
    /// Affected assemblies (used-by closure, shortest path each).
    pub affected: Vec<Affected>,
}

impl ImpactReport {
    /// Total affected assemblies.
    pub fn total(&self) -> usize {
        self.affected.len()
    }
    /// How many affected assemblies hold a Released revision.
    pub fn released_count(&self) -> usize {
        self.affected.iter().filter(|a| a.released).count()
    }
}

/// The PDM relationship graph.
///
/// Defaults to empty; build it with [`RelationshipGraph::from_oepgeo_document`]
/// (real oep-geo assembly data) or by hand with [`RelationshipGraph::add_item`]
/// and [`RelationshipGraph::add_usage`].
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RelationshipGraph {
    /// item id -> metadata.
    items: BTreeMap<String, ItemMeta>,
    /// parent -> outgoing usage edges.
    out: BTreeMap<String, Vec<UsageEdge>>,
    /// child -> incoming usage edges (the reverse index `where_used` walks).
    in_: BTreeMap<String, Vec<UsageEdge>>,
    /// item id -> recorded revisions (lifecycle state per revision).
    revisions: BTreeMap<String, Vec<RevisionRecord>>,
}

impl RelationshipGraph {
    /// An empty graph.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register (or update) an item node. Returns [`GraphError::MissingName`]
    /// if `name` is empty.
    pub fn add_item(
        &mut self,
        id: impl Into<String>,
        name: impl Into<String>,
        kind: ItemKind,
    ) -> Result<(), GraphError> {
        let id = id.into();
        let name = name.into();
        if name.is_empty() {
            return Err(GraphError::MissingName(id));
        }
        self.items.insert(id.clone(), ItemMeta { id, name, kind });
        Ok(())
    }

    /// Add a usage edge `parent` composes `child` with quantity `qty`.
    /// Aggregates duplicates: repeated `(parent, child)` pairs sum their
    /// quantities, so ingesting a sub-assembly with N instances of the same
    /// part yields one edge with `qty = N`.
    pub fn add_usage(
        &mut self,
        parent: impl Into<String>,
        child: impl Into<String>,
        qty: u32,
    ) -> Result<(), GraphError> {
        let parent = parent.into();
        let child = child.into();
        let self_out = self.out.entry(parent.clone()).or_default();
        if let Some(edge) = self_out
            .iter_mut()
            .find(|e| e.child == child && e.child_revision.is_none())
        {
            edge.qty = edge.qty.saturating_add(qty);
        } else {
            self_out.push(UsageEdge::new(parent.clone(), child.clone(), qty));
            self.in_
                .entry(child.clone())
                .or_default()
                .push(UsageEdge::new(parent.clone(), child, qty));
        }
        Ok(())
    }

    /// Record a revision + lifecycle state for an item.
    pub fn add_revision(
        &mut self,
        item: impl Into<String>,
        revision: impl Into<String>,
        state: LifecycleState,
    ) {
        self.revisions
            .entry(item.into())
            .or_default()
            .push(RevisionRecord {
                revision: revision.into(),
                state,
            });
    }

    /// Number of item nodes.
    pub fn node_count(&self) -> usize {
        self.items.len()
    }

    /// Number of usage edges (aggregated).
    pub fn edge_count(&self) -> usize {
        self.out.values().map(Vec::len).sum()
    }

    /// True if the graph has an item with this id.
    pub fn contains(&self, item: &str) -> bool {
        self.items.contains_key(item)
    }

    /// Item metadata, if present.
    pub fn item(&self, item: &str) -> Option<&ItemMeta> {
        self.items.get(item)
    }

    /// Recorded revisions for an item (in insertion order), if any.
    pub fn revisions(&self, item: &str) -> &[RevisionRecord] {
        self.revisions.get(item).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Whether the item has at least one `Released` revision.
    pub fn is_released(&self, item: &str) -> bool {
        self.revisions
            .get(item)
            .map(|rs| rs.iter().any(|r| r.state == LifecycleState::Released))
            .unwrap_or(false)
    }

    /// The released revision ids of an item (empty when none).
    pub fn released_revisions(&self, item: &str) -> Vec<String> {
        self.revisions
            .get(item)
            .map(|rs| {
                rs.iter()
                    .filter(|r| r.state == LifecycleState::Released)
                    .map(|r| r.revision.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Where is `item` used across assemblies?
    ///
    /// Returns every assembly that composes the item, directly or through any
    /// nesting of sub-assemblies, deduplicated per assembly (shortest path
    /// wins, BFS order). Unknown items and empty graphs yield an empty report.
    pub fn where_used(&self, item: &str) -> WhereUsedReport {
        let mut report = WhereUsedReport {
            item: item.to_string(),
            ..Default::default()
        };
        if !self.contains(item) {
            return report;
        }

        // BFS up the reverse (used-by) index, carrying qty product + path.
        // Each assembly is visited once (shortest path first).
        let mut visited: BTreeSet<String> = BTreeSet::new();
        let mut queue: VecDeque<(String, u32, Vec<String>)> = VecDeque::new();

        if let Some(incoming) = self.in_.get(item) {
            for edge in incoming {
                if visited.insert(edge.parent.clone()) {
                    queue.push_back((edge.parent.clone(), edge.qty, vec![edge.parent.clone()]));
                }
            }
        }

        while let Some((assembly, qty, path)) = queue.pop_front() {
            let depth = path.len();
            let hit = UsageHit {
                assembly: assembly.clone(),
                assembly_name: self
                    .items
                    .get(&assembly)
                    .map(|m| m.name.clone())
                    .unwrap_or_default(),
                path: path.clone(),
                depth,
                qty,
                direct: depth == 1,
            };
            if hit.direct {
                report.direct.push(hit);
            } else {
                report.transitive.push(hit);
            }

            if let Some(incoming) = self.in_.get(&assembly) {
                for edge in incoming {
                    if !visited.contains(&edge.parent) {
                        visited.insert(edge.parent.clone());
                        let mut new_path = vec![edge.parent.clone()];
                        new_path.extend(path.iter().cloned());
                        let new_qty = qty.saturating_mul(edge.qty);
                        queue.push_back((edge.parent.clone(), new_qty, new_path));
                    }
                }
            }
        }

        report
    }

    /// What does a revision change to `change.item` affect?
    ///
    /// Walks the full used-by closure of the changed item (the same reachable
    /// set as [`RelationshipGraph::where_used`]) and annotates each affected
    /// assembly with its release state, so "what a revision change affects"
    /// is answered all the way up to released top-level assemblies.
    pub fn impact(&self, change: &Change) -> ImpactReport {
        let seen = self.where_used(&change.item);
        let affected: Vec<Affected> = seen
            .all()
            .map(|hit| Affected {
                released: self.is_released(&hit.assembly),
                released_revisions: self.released_revisions(&hit.assembly),
                assembly: hit.assembly.clone(),
                assembly_name: hit.assembly_name.clone(),
                path: hit.path.clone(),
                depth: hit.depth,
                qty: hit.qty,
                direct: hit.direct,
            })
            .collect();
        ImpactReport {
            change: change.clone(),
            affected,
        }
    }

    /// Build a relationship graph from a real oep-geo assembly document (the
    /// P7 assembly workbench output format, [`Document`]).
    ///
    /// Maps every part to a `Part` node, every assembly to an `Assembly`
    /// node, and every instance to a composed-of edge (parent assembly ->
    /// source part or sub-assembly), aggregating quantity per identical
    /// source. Dangling instance references are reported as
    /// [`GraphError::DanglingReference`]; a non-`oep-geo` format / unsupported
    /// version yields [`GraphError::InvalidDocument`].
    pub fn from_oepgeo_document(doc: &Document) -> Result<Self, GraphError> {
        if doc.format != oep_geo::document::FORMAT {
            return Err(GraphError::InvalidDocument(format!(
                "expected format '{}', got '{}'",
                oep_geo::document::FORMAT,
                doc.format
            )));
        }
        if doc.version != oep_geo::document::VERSION {
            return Err(GraphError::InvalidDocument(format!(
                "unsupported version {}",
                doc.version
            )));
        }

        let mut g = RelationshipGraph::new();

        for part in &doc.parts {
            g.add_item(part.id.clone(), part.name.clone(), ItemKind::Part)
                .map_err(|_| GraphError::MissingName(part.id.clone()))?;
        }
        for asm in &doc.assemblies {
            g.add_item(asm.id.clone(), asm.name.clone(), ItemKind::Assembly)
                .map_err(|_| GraphError::MissingName(asm.id.clone()))?;
        }

        for asm in &doc.assemblies {
            // Aggregate instances by source id per assembly, then add one edge
            // per distinct source with the summed placement count.
            let mut counts: BTreeMap<&str, u32> = BTreeMap::new();
            for inst in &asm.instances {
                let target = inst.source.referenced_id();
                *counts.entry(target).or_insert(0) += 1;
            }
            for (target, qty) in counts {
                if !g.contains(target) {
                    return Err(GraphError::DanglingReference {
                        parent: asm.id.clone(),
                        missing: target.to_string(),
                    });
                }
                g.add_usage(asm.id.clone(), target.to_string(), qty)?;
            }
        }

        Ok(g)
    }

    /// The direct children (composed-of edges) of `parent`, in insertion
    /// order. This is the BOM walk: every item `parent` directly composes,
    /// with its quantity. Empty for a leaf item or an unknown id.
    pub fn children(&self, parent: &str) -> Vec<UsageEdge> {
        self.out.get(parent).cloned().unwrap_or_default()
    }

    /// The BOM roots: items that nothing else composes (top-level assemblies
    /// and any orphan parts), sorted by id. These are the natural starting
    /// points for a whole-product BOM tree.
    pub fn roots(&self) -> Vec<String> {
        self.items
            .keys()
            .filter(|id| !self.in_.contains_key(*id))
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    fn two_part_graph() -> RelationshipGraph {
        let json = include_str!("../tests/golden/two-part-fixture.oepgeo.json");
        let doc = oep_geo::Document::from_json_pretty(json).unwrap();
        RelationshipGraph::from_oepgeo_document(&doc).unwrap()
    }

    #[test]
    fn ingest_maps_parts_assemblies_and_instances() {
        let g = two_part_graph();
        assert_eq!(g.node_count(), 3);
        assert_eq!(g.item("oep-part-base").unwrap().kind, ItemKind::Part);
        assert_eq!(
            g.item("oep-assembly-fixture").unwrap().kind,
            ItemKind::Assembly
        );
        assert_eq!(g.edge_count(), 2);
        // fixture composes cap once
        assert!(g
            .out
            .get("oep-assembly-fixture")
            .unwrap()
            .iter()
            .any(|e| e.child == "oep-part-cap" && e.qty == 1));
        // the reverse (used-by) index agrees
        assert!(g
            .in_
            .get("oep-part-cap")
            .unwrap()
            .iter()
            .any(|e| e.parent == "oep-assembly-fixture"));
    }

    #[test]
    fn add_usage_aggregates_identical_pairs() {
        let mut g = RelationshipGraph::new();
        g.add_item("asm", "Asm", ItemKind::Assembly).unwrap();
        g.add_item("part", "Part", ItemKind::Part).unwrap();
        g.add_usage("asm", "part", 1).unwrap();
        g.add_usage("asm", "part", 3).unwrap();
        assert_eq!(g.edge_count(), 1);
        let e = &g.out["asm"][0];
        assert_eq!(e.qty, 4, "repeated identical usage sums qty");
    }

    #[test]
    fn ingest_rejects_dangling_references() {
        let mut doc = oep_geo::Document::new("mm");
        let asm =
            oep_geo::Assembly::new("oep-assembly-x", "X").with_instance(oep_geo::Instance::new(
                "inst-1",
                "ghost",
                oep_geo::InstanceSource::Part {
                    id: "part-missing".into(),
                },
            ));
        doc.push_assembly(asm);
        let err = RelationshipGraph::from_oepgeo_document(&doc).unwrap_err();
        match err {
            GraphError::DanglingReference { parent, missing } => {
                assert_eq!(parent, "oep-assembly-x");
                assert_eq!(missing, "part-missing");
            }
            other => panic!("expected DanglingReference, got {other:?}"),
        }
    }

    #[test]
    fn ingest_rejects_wrong_format_or_version() {
        let mut doc = oep_geo::Document::new("mm");
        doc.format = "not-oep-geo".into();
        let err = RelationshipGraph::from_oepgeo_document(&doc).unwrap_err();
        assert!(matches!(err, GraphError::InvalidDocument(_)));

        let mut doc = oep_geo::Document::new("mm");
        doc.version = 99;
        let err = RelationshipGraph::from_oepgeo_document(&doc).unwrap_err();
        assert!(matches!(err, GraphError::InvalidDocument(_)));
    }

    #[test]
    fn add_item_rejects_empty_name() {
        let mut g = RelationshipGraph::new();
        let err = g.add_item("id", "", ItemKind::Part).unwrap_err();
        assert_eq!(err, GraphError::MissingName("id".into()));
    }

    #[test]
    fn graph_and_reports_serde_round_trip() {
        let g = two_part_graph();
        let json = serde_json::to_string(&g).unwrap();
        let back: RelationshipGraph = serde_json::from_str(&json).unwrap();
        assert_eq!(back.edge_count(), g.edge_count());
        assert_eq!(
            back.where_used("oep-part-base"),
            g.where_used("oep-part-base")
        );

        let rep = g.impact(&Change::new("oep-part-base", "1", "2"));
        let rj = serde_json::to_string(&rep).unwrap();
        let rb: ImpactReport = serde_json::from_str(&rj).unwrap();
        assert_eq!(rb.total(), rep.total());
    }

    #[test]
    fn impact_on_unknown_item_is_empty() {
        let g = two_part_graph();
        let rep = g.impact(&Change::new("no-such", "1", "2"));
        assert_eq!(rep.total(), 0);
        assert_eq!(rep.released_count(), 0);
    }

    #[test]
    fn released_revisions_lookup() {
        let mut g = RelationshipGraph::new();
        g.add_revision("asm", "1", LifecycleState::Working);
        g.add_revision("asm", "2", LifecycleState::Released);
        g.add_revision("asm", "3", LifecycleState::UnderReview);
        assert!(g.is_released("asm"));
        assert_eq!(g.released_revisions("asm"), vec!["2"]);
        assert_eq!(g.revisions("asm").len(), 3);
        assert!(!g.is_released("missing"));
    }

    #[test]
    fn multiple_direct_parents_reported() {
        let mut g = RelationshipGraph::new();
        g.add_item("part", "Part", ItemKind::Part).unwrap();
        g.add_item("asm-a", "A", ItemKind::Assembly).unwrap();
        g.add_item("asm-b", "B", ItemKind::Assembly).unwrap();
        g.add_usage("asm-a", "part", 2).unwrap();
        g.add_usage("asm-b", "part", 5).unwrap();
        let wu = g.where_used("part");
        assert_eq!(wu.direct.len(), 2);
        let total: u32 = wu.direct.iter().map(|h| h.qty).sum();
        assert_eq!(total, 7);
    }

    #[test]
    fn children_accessor_walks_bom_top_down() {
        let g = two_part_graph();
        // fixture composes base + cap (top-down BOM walk).
        let kids = g.children("oep-assembly-fixture");
        assert_eq!(kids.len(), 2);
        let names: Vec<&str> = kids.iter().map(|e| e.child.as_str()).collect();
        assert!(names.contains(&"oep-part-base"));
        assert!(names.contains(&"oep-part-cap"));
        // a leaf part has no children.
        assert!(g.children("oep-part-base").is_empty());
        // unknown id yields empty (tolerant).
        assert!(g.children("nope").is_empty());
    }

    #[test]
    fn roots_accessor_reports_top_level_items() {
        let g = two_part_graph();
        let roots = g.roots();
        assert_eq!(roots, vec!["oep-assembly-fixture".to_string()]);
        // empty graph has no roots.
        let empty = RelationshipGraph::new();
        assert!(empty.roots().is_empty());
    }

    #[test]
    fn nested_children_and_roots_on_real_bracket() {
        let json = include_str!("../../oep-geo/tests/golden/example-bracket.oepgeo.json");
        let doc = oep_geo::Document::from_json_pretty(json).unwrap();
        let g = RelationshipGraph::from_oepgeo_document(&doc).unwrap();
        // Top assembly composes bracket + fastener-set.
        let top = g.children("oep-assembly-bracket-assembly");
        assert_eq!(top.len(), 2);
        // Fastener-set composes 4 fasteners.
        let set = g.children("oep-assembly-fastener-set");
        assert_eq!(set.len(), 1);
        assert_eq!(set[0].child, "oep-part-fastener");
        assert_eq!(set[0].qty, 4);
        // The bracket assembly is the only root.
        assert_eq!(g.roots(), vec!["oep-assembly-bracket-assembly".to_string()]);
    }
}

//! Research-only flat canonical partitions with a resident match cache.
//!
//! This oracle tests a two-level physical representation that retains one
//! semantic state. A partition persists as a sorted flat array of canonical
//! Concepts. Exact lookup uses that array directly. A running server may derive
//! a densely connected trie from the same records for subset and superset
//! traversal, or fall back to scanning the array while the cache is cold.
//!
//! The experiment is deliberately narrower than Pangine's current matcher. It
//! does not yet admit Correlations, Observations, nested collections, Percepts,
//! or non-default Relevance. Its purpose is to test the storage direction before
//! turning any particular cache layout into an architectural assumption.

use super::super::*;

#[derive(Default)]
struct ResidentMatchCache {
    root: CanonicalSetNode,
}

struct CanonicalPartition {
    concepts: Vec<ConceptId>,
}

#[derive(Default)]
struct CanonicalSetNode {
    concepts: ConceptMap,
    children: BTreeMap<String, CanonicalSetNode>,
}

struct Retrieval {
    concepts: ConceptMap,
    visited_nodes: usize,
}

impl CanonicalPartition {
    fn from_concepts(pangine: &Pangine, concepts: impl IntoIterator<Item = ConceptId>) -> Result<Self, &'static str> {
        let mut concepts = concepts.into_iter().collect::<Vec<_>>();
        for concept in &concepts {
            flat_canonical_keys(pangine, concept)?;
        }
        concepts.sort_by_cached_key(|concept| pangine.format_concept(concept, false));
        concepts.dedup_by(|left, right| pangine.format_concept(left, false) == pangine.format_concept(right, false));
        Ok(Self { concepts })
    }

    fn exact(&self, pangine: &Pangine, query: &ConceptId) -> Result<Retrieval, &'static str> {
        flat_canonical_keys(pangine, query)?;
        let query_key = pangine.format_concept(query, false);
        let mut visited_nodes = 0;
        let found = self.concepts.binary_search_by(|concept| {
            visited_nodes += 1;
            pangine.format_concept(concept, false).cmp(&query_key)
        });
        let concepts = found.map(|index| ConceptMap::from([(self.concepts[index].clone(), Relevance::DEFAULT)])).unwrap_or_default();
        Ok(Retrieval { concepts, visited_nodes })
    }

    fn scan_subsets(&self, pangine: &Pangine, query: &ConceptId) -> Result<Retrieval, &'static str> {
        self.scan_containment(pangine, query, false)
    }

    fn scan_supersets(&self, pangine: &Pangine, query: &ConceptId) -> Result<Retrieval, &'static str> {
        self.scan_containment(pangine, query, true)
    }

    fn scan_containment(&self, pangine: &Pangine, query: &ConceptId, stored_is_superset: bool) -> Result<Retrieval, &'static str> {
        let query = flat_canonical_keys(pangine, query)?;
        let mut concepts = ConceptMap::new();
        for concept in &self.concepts {
            let stored = flat_canonical_keys(pangine, concept)?;
            let matches = if stored_is_superset { ordered_subset(&query, &stored) } else { ordered_subset(&stored, &query) };
            if matches {
                concepts.insert(concept.clone(), Relevance::DEFAULT);
            }
        }
        Ok(Retrieval { concepts, visited_nodes: self.concepts.len() })
    }

    fn contains(&self, pangine: &Pangine, concept: &ConceptId) -> bool {
        let key = pangine.format_concept(concept, false);
        self.concepts.binary_search_by(|candidate| pangine.format_concept(candidate, false).cmp(&key)).is_ok()
    }
}

impl ResidentMatchCache {
    fn from_partition(pangine: &Pangine, partition: &CanonicalPartition) -> Result<Self, &'static str> {
        let mut cache = Self::default();
        for concept in &partition.concepts {
            cache.insert(pangine, concept.clone())?;
        }
        Ok(cache)
    }

    fn insert(&mut self, pangine: &Pangine, concept: ConceptId) -> Result<bool, &'static str> {
        let mut node = &mut self.root;
        for key in flat_canonical_keys(pangine, &concept)? {
            node = node.children.entry(key).or_default();
        }
        Ok(node.concepts.insert(concept, Relevance::DEFAULT).is_none())
    }

    fn subsets(&self, pangine: &Pangine, partition: &CanonicalPartition, query: &ConceptId) -> Result<Retrieval, &'static str> {
        let keys = flat_canonical_keys(pangine, query)?;
        let mut retrieval = Retrieval { concepts: ConceptMap::new(), visited_nodes: 0 };
        Self::collect_subsets(&self.root, &keys, 0, &mut retrieval);
        retrieval.concepts.retain(|concept, _| partition.contains(pangine, concept));
        Ok(retrieval)
    }

    fn supersets(&self, pangine: &Pangine, partition: &CanonicalPartition, query: &ConceptId) -> Result<Retrieval, &'static str> {
        let keys = flat_canonical_keys(pangine, query)?;
        let mut retrieval = Retrieval { concepts: ConceptMap::new(), visited_nodes: 0 };
        Self::collect_supersets(&self.root, &keys, 0, &mut retrieval);
        retrieval.concepts.retain(|concept, _| partition.contains(pangine, concept));
        Ok(retrieval)
    }

    fn collect_subsets(node: &CanonicalSetNode, query: &[String], start: usize, retrieval: &mut Retrieval) {
        retrieval.visited_nodes += 1;
        extend_concepts(&mut retrieval.concepts, &node.concepts);

        for index in start..query.len() {
            if let Some(child) = node.children.get(&query[index]) {
                Self::collect_subsets(child, query, index + 1, retrieval);
            }
        }
    }

    fn collect_supersets(node: &CanonicalSetNode, query: &[String], index: usize, retrieval: &mut Retrieval) {
        retrieval.visited_nodes += 1;
        if index == query.len() {
            extend_concepts(&mut retrieval.concepts, &node.concepts);
            for child in node.children.values() {
                Self::collect_all(child, retrieval);
            }
            return;
        }

        let required = &query[index];
        for (key, child) in node.children.range(..=required.clone()) {
            let next_index = index + usize::from(key == required);
            Self::collect_supersets(child, query, next_index, retrieval);
        }
    }

    fn collect_all(node: &CanonicalSetNode, retrieval: &mut Retrieval) {
        retrieval.visited_nodes += 1;
        extend_concepts(&mut retrieval.concepts, &node.concepts);
        for child in node.children.values() {
            Self::collect_all(child, retrieval);
        }
    }
}

fn ordered_subset(subset: &[String], superset: &[String]) -> bool {
    let mut subset_index = 0;
    let mut superset_index = 0;
    while subset_index < subset.len() && superset_index < superset.len() {
        match subset[subset_index].cmp(&superset[superset_index]) {
            Ordering::Less => return false,
            Ordering::Equal => subset_index += 1,
            Ordering::Greater => {}
        }
        superset_index += 1;
    }
    subset_index == subset.len()
}

fn flat_canonical_keys(pangine: &Pangine, concept: &ConceptId) -> Result<Vec<String>, &'static str> {
    match &concept.0.kind {
        ConceptKind::Named(_) => Ok(vec![pangine.format_concept(concept, false)]),
        ConceptKind::Relevance => {
            let mut keys = Vec::with_capacity(concept.0.subconcepts.len());
            for (child, relevance) in &concept.0.subconcepts {
                if *relevance != Relevance::DEFAULT || !matches!(child.0.kind, ConceptKind::Named(_)) {
                    return Err("canonical flat store requires default-relevance Named members");
                }
                keys.push(pangine.format_concept(child, false));
            }
            keys.sort();
            Ok(keys)
        }
        _ => Err("canonical flat store does not yet support recursive Concept structure"),
    }
}

fn extend_concepts(target: &mut ConceptMap, source: &ConceptMap) {
    for (concept, relevance) in source {
        target.insert(concept.clone(), *relevance);
    }
}

fn must_reference(pangine: &mut Pangine, source: &str) -> ConceptId {
    pangine.reference_concept(source).unwrap().unwrap_or_else(|| panic!("expected a Concept from {source:?}"))
}

fn formatted_set(pangine: &Pangine, concepts: &ConceptMap) -> BTreeSet<String> {
    concepts.keys().map(|concept| pangine.format_concept(concept, false)).collect()
}

#[test]
fn flat_array_and_resident_cache_agree_on_exact_subset_and_superset_retrieval() {
    let mut pangine = Pangine::new();
    let concepts = ["[A]", "[A][B]", "[A][B][C]", "[B][C]", "[D]"].into_iter().map(|source| must_reference(&mut pangine, source)).collect::<Vec<_>>();
    let partition = CanonicalPartition::from_concepts(&pangine, concepts).unwrap();
    let cache = ResidentMatchCache::from_partition(&pangine, &partition).unwrap();

    let ab = must_reference(&mut pangine, "[A][B]");
    let abc = must_reference(&mut pangine, "[A][B][C]");
    let exact = partition.exact(&pangine, &ab).unwrap();
    let cold_supersets = partition.scan_supersets(&pangine, &ab).unwrap();
    let hot_supersets = cache.supersets(&pangine, &partition, &ab).unwrap();
    let cold_subsets = partition.scan_subsets(&pangine, &abc).unwrap();
    let hot_subsets = cache.subsets(&pangine, &partition, &abc).unwrap();

    assert_eq!(formatted_set(&pangine, &exact.concepts), BTreeSet::from(["[A][B]".to_owned()]));
    assert_eq!(cold_supersets.concepts, hot_supersets.concepts);
    assert_eq!(formatted_set(&pangine, &hot_supersets.concepts), BTreeSet::from(["[A][B]".to_owned(), "[A][B][C]".to_owned()]));
    assert_eq!(cold_subsets.concepts, hot_subsets.concepts);
    assert_eq!(
        formatted_set(&pangine, &hot_subsets.concepts),
        BTreeSet::from(["[A]".to_owned(), "[A][B]".to_owned(), "[A][B][C]".to_owned(), "[B][C]".to_owned()])
    );
}

#[test]
fn flat_partitions_map_and_reduce_matches_and_degrade_by_source_subset() {
    let mut pangine = Pangine::new();
    let ab = must_reference(&mut pangine, "[A][B]");
    let ac = must_reference(&mut pangine, "[A][C]");
    let noise = must_reference(&mut pangine, "[D][E]");
    let query = must_reference(&mut pangine, "[A]");
    let partition_a = CanonicalPartition::from_concepts(&pangine, [ab.clone()]).unwrap();
    let partition_b = CanonicalPartition::from_concepts(&pangine, [ac.clone()]).unwrap();
    let partition_c = CanonicalPartition::from_concepts(&pangine, [noise.clone()]).unwrap();
    let combined = CanonicalPartition::from_concepts(&pangine, [ab, ac.clone(), noise]).unwrap();
    let cache_a = ResidentMatchCache::from_partition(&pangine, &partition_a).unwrap();
    let cache_b = ResidentMatchCache::from_partition(&pangine, &partition_b).unwrap();
    let cache_c = ResidentMatchCache::from_partition(&pangine, &partition_c).unwrap();
    let combined_cache = ResidentMatchCache::from_partition(&pangine, &combined).unwrap();

    let mapped_a = cache_a.supersets(&pangine, &partition_a, &query).unwrap().concepts;
    let mapped_b = cache_b.supersets(&pangine, &partition_b, &query).unwrap().concepts;
    let mapped_c = cache_c.supersets(&pangine, &partition_c, &query).unwrap().concepts;
    let expected = combined_cache.supersets(&pangine, &combined, &query).unwrap().concepts;
    let mut reduced = ConceptMap::new();
    extend_concepts(&mut reduced, &mapped_a);
    extend_concepts(&mut reduced, &mapped_b);
    extend_concepts(&mut reduced, &mapped_c);
    extend_concepts(&mut reduced, &mapped_a);

    assert_eq!(reduced, expected);
    assert_eq!(formatted_set(&pangine, &reduced), BTreeSet::from(["[A][B]".to_owned(), "[A][C]".to_owned()]));

    let mut degraded = ConceptMap::new();
    extend_concepts(&mut degraded, &mapped_a);
    extend_concepts(&mut degraded, &mapped_c);
    assert_eq!(formatted_set(&pangine, &degraded), BTreeSet::from(["[A][B]".to_owned()]));
    assert!(!degraded.contains_key(&ac));
}

#[test]
fn discarded_rebuilt_and_stale_caches_remain_subordinate_to_the_flat_partition() {
    let mut pangine = Pangine::new();
    let ab = must_reference(&mut pangine, "[A][B]");
    let ac = must_reference(&mut pangine, "[A][C]");
    let query = must_reference(&mut pangine, "[A]");
    let combined = CanonicalPartition::from_concepts(&pangine, [ab.clone(), ac]).unwrap();
    let remaining = CanonicalPartition::from_concepts(&pangine, [ab]).unwrap();
    let stale = ResidentMatchCache::from_partition(&pangine, &combined).unwrap();
    let rebuilt = ResidentMatchCache::from_partition(&pangine, &remaining).unwrap();
    let cold = remaining.scan_supersets(&pangine, &query).unwrap();
    let stale_result = stale.supersets(&pangine, &remaining, &query).unwrap();
    let rebuilt_result = rebuilt.supersets(&pangine, &remaining, &query).unwrap();

    assert_eq!(stale_result.concepts, cold.concepts);
    assert_eq!(rebuilt_result.concepts, cold.concepts);
    assert_eq!(formatted_set(&pangine, &cold.concepts), BTreeSet::from(["[A][B]".to_owned()]));
}

#[test]
fn one_resident_canonical_order_prunes_selectively_but_not_uniformly() {
    let mut pangine = Pangine::new();
    let mut concepts = Vec::new();
    for index in 0..512 {
        concepts.push(must_reference(&mut pangine, &format!("[N-{index:04}][Y-{index:04}]")));
    }
    for source in ["[A-anchor][answer-low]", "[Z-anchor][answer-high]"] {
        concepts.push(must_reference(&mut pangine, source));
    }
    let partition = CanonicalPartition::from_concepts(&pangine, concepts).unwrap();
    let cache = ResidentMatchCache::from_partition(&pangine, &partition).unwrap();

    let low_query = must_reference(&mut pangine, "[A-anchor]");
    let high_query = must_reference(&mut pangine, "[Z-anchor]");
    let low = cache.supersets(&pangine, &partition, &low_query).unwrap();
    let high = cache.supersets(&pangine, &partition, &high_query).unwrap();

    assert_eq!(formatted_set(&pangine, &low.concepts), BTreeSet::from(["[A-anchor][answer-low]".to_owned()]));
    assert_eq!(formatted_set(&pangine, &high.concepts), BTreeSet::from(["[Z-anchor][answer-high]".to_owned()]));
    assert!(low.visited_nodes < 10);
    assert!(high.visited_nodes > 1_000);
}

#[test]
fn flat_oracle_does_not_claim_recursive_question_coverage() {
    let mut pangine = Pangine::new();
    let correlation = must_reference(&mut pangine, "{[A]->[B]}");

    assert!(matches!(CanonicalPartition::from_concepts(&pangine, [correlation]), Err("canonical flat store does not yet support recursive Concept structure")));
}

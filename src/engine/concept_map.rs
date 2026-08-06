use super::ConceptId;
use crate::Relevance;
use im::vector::{ConsumingIter, Iter as VectorIter};
use im::Vector;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::ops::Index;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct LookupSummary {
    entries: usize,
    sum: u64,
    xor: u64,
}

impl LookupSummary {
    fn add(&mut self, concept: &ConceptId, relevance: Relevance) {
        let entry = entry_fingerprint(concept, relevance);
        self.entries += 1;
        self.sum = self.sum.wrapping_add(entry);
        self.xor ^= entry.rotate_left((entry & 63) as u32);
    }

    fn remove(&mut self, concept: &ConceptId, relevance: Relevance) {
        let entry = entry_fingerprint(concept, relevance);
        self.entries -= 1;
        self.sum = self.sum.wrapping_sub(entry);
        self.xor ^= entry.rotate_left((entry & 63) as u32);
    }

    fn hash<H: Hasher>(self, hasher: &mut H) {
        self.entries.hash(hasher);
        self.sum.hash(hasher);
        self.xor.hash(hasher);
    }
}

/// An ordered immutable map whose clones reuse unchanged sequence chunks.
///
/// All mutation stays behind this type so its order-independent canonical
/// lookup summary cannot become stale.
#[derive(Clone, Debug, Default)]
pub(super) struct ConceptMap {
    entries: Vector<(ConceptId, Relevance)>,
    summary: LookupSummary,
}

pub(super) struct Iter<'a> {
    entries: VectorIter<'a, (ConceptId, Relevance)>,
}

impl<'a> Iterator for Iter<'a> {
    type Item = (&'a ConceptId, &'a Relevance);

    fn next(&mut self) -> Option<Self::Item> {
        self.entries.next().map(|entry| (&entry.0, &entry.1))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.entries.size_hint()
    }
}

impl DoubleEndedIterator for Iter<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.entries.next_back().map(|entry| (&entry.0, &entry.1))
    }
}

impl ExactSizeIterator for Iter<'_> {}

impl ConceptMap {
    pub(super) fn new() -> Self {
        Self::default()
    }

    pub(super) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(super) fn len(&self) -> usize {
        self.entries.len()
    }

    pub(super) fn first_key_value(&self) -> Option<(&ConceptId, &Relevance)> {
        self.entries.front().map(|entry| (&entry.0, &entry.1))
    }

    pub(super) fn iter(&self) -> Iter<'_> {
        Iter { entries: self.entries.iter() }
    }

    pub(super) fn keys(&self) -> impl DoubleEndedIterator<Item = &ConceptId> + ExactSizeIterator {
        self.entries.iter().map(|entry| &entry.0)
    }

    pub(super) fn values(&self) -> impl DoubleEndedIterator<Item = &Relevance> + ExactSizeIterator {
        self.entries.iter().map(|entry| &entry.1)
    }

    pub(super) fn contains_key(&self, concept: &ConceptId) -> bool {
        self.position(concept).is_ok()
    }

    pub(super) fn insert(&mut self, concept: ConceptId, relevance: Relevance) -> Option<Relevance> {
        match self.position(&concept) {
            Ok(index) => {
                let previous = self.entries[index].1;
                self.summary.remove(&concept, previous);
                self.summary.add(&concept, relevance);
                let replaced = self.entries.set(index, (concept, relevance));
                debug_assert_eq!(replaced.1, previous);
                Some(previous)
            }
            Err(index) => {
                self.summary.add(&concept, relevance);
                self.entries.insert(index, (concept, relevance));
                None
            }
        }
    }

    pub(super) fn remove(&mut self, concept: &ConceptId) -> Option<Relevance> {
        let index = self.position(concept).ok()?;
        let removed = self.entries.remove(index);
        self.summary.remove(&removed.0, removed.1);
        Some(removed.1)
    }

    pub(super) fn hash_lookup_summary<H: Hasher>(&self, hasher: &mut H) {
        self.summary.hash(hasher);
    }

    fn position(&self, concept: &ConceptId) -> Result<usize, usize> {
        self.entries.binary_search_by(|entry| entry.0.cmp(concept))
    }
}

impl PartialEq for ConceptMap {
    fn eq(&self, other: &Self) -> bool {
        self.entries == other.entries
    }
}

impl Index<&ConceptId> for ConceptMap {
    type Output = Relevance;

    fn index(&self, concept: &ConceptId) -> &Self::Output {
        let index = self.position(concept).unwrap_or_else(|_| panic!("ConceptMap does not contain {concept:?}"));
        &self.entries[index].1
    }
}

impl FromIterator<(ConceptId, Relevance)> for ConceptMap {
    fn from_iter<T: IntoIterator<Item = (ConceptId, Relevance)>>(entries: T) -> Self {
        let mut map = Self::new();
        for (concept, relevance) in entries {
            map.insert(concept, relevance);
        }
        map
    }
}

impl<const SIZE: usize> From<[(ConceptId, Relevance); SIZE]> for ConceptMap {
    fn from(entries: [(ConceptId, Relevance); SIZE]) -> Self {
        entries.into_iter().collect()
    }
}

impl<'a> IntoIterator for &'a ConceptMap {
    type Item = (&'a ConceptId, &'a Relevance);
    type IntoIter = Iter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl IntoIterator for ConceptMap {
    type Item = (ConceptId, Relevance);
    type IntoIter = ConsumingIter<(ConceptId, Relevance)>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.into_iter()
    }
}

fn entry_fingerprint(concept: &ConceptId, relevance: Relevance) -> u64 {
    let mut hasher = DefaultHasher::new();
    concept.hash(&mut hasher);
    hash_float(relevance.x_coefficient, &mut hasher);
    hasher.finish()
}

fn hash_float<H: Hasher>(value: f32, hasher: &mut H) {
    // f32 equality treats positive and negative zero as equal, so canonical
    // lookup must put them in the same candidate bucket.
    if value == 0.0 {
        0_u32.hash(hasher);
    } else {
        value.to_bits().hash(hasher);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Pangine;

    #[test]
    fn mutations_keep_lookup_summary_equal_to_a_complete_rebuild() {
        let mut pangine = Pangine::new();
        let first = pangine.reference_named("first").unwrap();
        let second = pangine.reference_named("second").unwrap();
        let third = pangine.reference_named("third").unwrap();
        let entries = [(first, Relevance::DEFAULT), (second, Relevance::new(2.0)), (third, Relevance::new(-3.0))];

        let forward = ConceptMap::from(entries.clone());
        let reversed = ConceptMap::from([entries[2].clone(), entries[1].clone(), entries[0].clone()]);
        assert_eq!(forward, reversed);
        assert_eq!(forward.summary, reversed.summary);

        let mut modified = forward.clone();
        modified.insert(entries[0].0.clone(), Relevance::new(4.0));
        modified.remove(&entries[1].0);
        let rebuilt = modified.iter().map(|(concept, &relevance)| (concept.clone(), relevance)).collect::<ConceptMap>();
        assert_eq!(modified, rebuilt);
        assert_eq!(modified.summary, rebuilt.summary);
    }

    #[test]
    fn cloned_snapshot_keeps_its_old_value_and_initially_shares_storage() {
        let mut pangine = Pangine::new();
        let concepts = (0..128).map(|index| pangine.reference_named(&format!("member-{index}")).unwrap()).collect::<Vec<_>>();
        let extra = pangine.reference_named("extra").unwrap();

        let mut current = concepts.into_iter().map(|concept| (concept, Relevance::DEFAULT)).collect::<ConceptMap>();
        let snapshot = current.clone();
        assert!(current.entries.ptr_eq(&snapshot.entries));

        current.insert(extra.clone(), Relevance::DEFAULT);
        assert!(!snapshot.contains_key(&extra));
        assert_eq!(snapshot.len(), 128);
        assert_eq!(current.len(), 129);
    }

    #[test]
    fn signed_zero_has_the_same_lookup_summary() {
        let mut pangine = Pangine::new();
        let concept = pangine.reference_named("member").unwrap();
        let positive = ConceptMap::from([(concept.clone(), Relevance::new(0.0))]);
        let negative = ConceptMap::from([(concept, Relevance::new(-0.0))]);

        assert_eq!(positive, negative);
        assert_eq!(positive.summary, negative.summary);
    }
}

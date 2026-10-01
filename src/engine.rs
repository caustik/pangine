use crate::Relevance;
use std::cell::Cell;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};
use std::rc::{Rc, Weak};
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

mod answer;
mod choice;
mod completion;
mod concept_answer;
mod concept_map;
mod console;
mod evaluation;
mod format;
mod interning;
mod parser;
mod question;
mod question_index;

pub use answer::{Answer, AnswerChoice, AnswerPossibility, AnswerSource, AnswerSupport, AnswerView};
pub use completion::{Completion, CompletionEvidence, CompletionRemainder, CompletionRemainderSide, CompletionResult};
use completion::{CompletionBindingOrigin, CompletionOrderedStep, CompletionOrderedWindow, CompletionRoute};
use concept_answer::{ConceptAnswer, LiveConceptAnswer};
use concept_map::ConceptMap;
pub use parser::{ParseError, ParseResult};
use question_index::PerceptQuestionIndex;

type CompositeLookup = BTreeMap<u64, Vec<Weak<Concept>>>;
type ProjectionAssignment = BTreeMap<ConceptId, ConceptId>;
/// The signed factor and distinct sources that identify one weighed derivation.
type DerivationSources = (Relevance, BTreeSet<QuestionSource>);
/// Each projected value's derivations and their weights summed across rows.
type CompletionProjectionSupport = BTreeMap<ConceptId, BTreeMap<DerivationSources, Relevance>>;
type QuestionSourceViewKey = (QuestionSource, ConceptId, BTreeMap<ConceptId, ConceptId>);
type QuestionSourceViews = BTreeMap<QuestionSourceViewKey, BTreeSet<CompletionRoute>>;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ConceptShape {
    Named,
    Percept,
    Unordered,
    Ordered(usize),
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
enum QuestionSourceOrigin {
    Percept(ConceptId),
    Subject,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct QuestionSource {
    origin: QuestionSourceOrigin,
    concept: ConceptId,
    relevance: Relevance,
}

impl QuestionSource {
    fn from_percept(percept: ConceptId, concept: ConceptId, relevance: Relevance) -> Self {
        Self { origin: QuestionSourceOrigin::Percept(percept), concept, relevance }
    }

    fn from_subject(subject: ConceptId) -> Self {
        Self { origin: QuestionSourceOrigin::Subject, concept: subject, relevance: Relevance::DEFAULT }
    }

    fn subject(&self) -> &ConceptId {
        match &self.origin {
            QuestionSourceOrigin::Percept(percept) => percept,
            QuestionSourceOrigin::Subject => &self.concept,
        }
    }

    fn percept(&self) -> Option<&ConceptId> {
        match &self.origin {
            QuestionSourceOrigin::Percept(percept) => Some(percept),
            QuestionSourceOrigin::Subject => None,
        }
    }
}

enum QuestionSelector {
    Percepts(Vec<ConceptId>),
    Subject(ConceptId),
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct QuestionSourceView {
    source: QuestionSource,
    matched: ConceptId,
    routes: BTreeSet<CompletionRoute>,
}

type QuestionSnapshot = QuestionSourceViews;

static NEXT_PANGINE_ID: AtomicUsize = AtomicUsize::new(0);

/// The reserved name of the global percept.
pub const GLOBAL_PERCEPT_NAME: &str = "*";

/// An error produced while composing an ordinary Concept from existing
/// engine-owned Concept handles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ConceptConstructionError {
    /// At least one supplied handle belongs to a different engine.
    ForeignConcept,
    /// Coefficient normalization exceeded the signed 64-bit relevance range.
    RelevanceOverflow,
}

impl std::fmt::Display for ConceptConstructionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ForeignConcept => formatter.write_str("concept belongs to a different engine"),
            Self::RelevanceOverflow => formatter.write_str("relevance coefficient exceeds the signed 64-bit range"),
        }
    }
}

impl std::error::Error for ConceptConstructionError {}

/// An error produced while replacing several Percept values as one update.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum PerceptUpdateError {
    /// A supplied Percept is foreign, ordinary, or the read-only global Percept.
    InvalidPercept,
    /// A supplied value belongs to a different engine.
    ForeignConcept,
    /// The same Percept appears more than once in the update.
    DuplicatePercept,
}

impl std::fmt::Display for PerceptUpdateError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidPercept => formatter.write_str("concept is not a mutable Percept owned by this engine"),
            Self::ForeignConcept => formatter.write_str("Percept value belongs to a different engine"),
            Self::DuplicatePercept => formatter.write_str("Percept appears more than once in the update"),
        }
    }
}

impl std::error::Error for PerceptUpdateError {}

/// An engine-scoped handle to an interned concept.
#[derive(Clone)]
pub struct ConceptId(Rc<Concept>);

impl ConceptId {
    fn new(pangine_id: usize, index: usize, kind: ConceptKind, subconcepts: ConceptMap) -> Self {
        Self(Rc::new(Concept { pangine_id, index, kind, subconcepts }))
    }

    fn key(&self) -> (usize, usize) {
        (self.0.pangine_id, self.0.index)
    }

    /// Returns the concept's allocation index within its owning engine.
    pub fn index(&self) -> usize {
        self.0.index
    }
}

impl std::fmt::Debug for ConceptId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_tuple("ConceptId").field(&self.0.index).finish()
    }
}

impl PartialEq for ConceptId {
    fn eq(&self, other: &Self) -> bool {
        self.key() == other.key()
    }
}

impl Eq for ConceptId {}

impl PartialOrd for ConceptId {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ConceptId {
    fn cmp(&self, other: &Self) -> Ordering {
        self.key().cmp(&other.key())
    }
}

impl Hash for ConceptId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.key().hash(state);
    }
}

/// The structural kind of an interned concept.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConceptKind {
    /// A named concept.
    Named(String),
    /// A mutable percept reference.
    Percept {
        /// The percept name.
        name: String,
    },
    /// An unordered composition whose member edges carry signed `x` coefficients.
    Unordered,
    /// An ordered composition whose component occurrences retain their positions.
    Ordered {
        /// The ordered component occurrences.
        components: Vec<ConceptId>,
    },
}

struct Concept {
    pangine_id: usize,
    index: usize,
    kind: ConceptKind,
    subconcepts: ConceptMap,
}

impl Concept {
    fn shape(&self) -> ConceptShape {
        match &self.kind {
            ConceptKind::Named(_) => ConceptShape::Named,
            ConceptKind::Percept { .. } => ConceptShape::Percept,
            ConceptKind::Unordered => ConceptShape::Unordered,
            ConceptKind::Ordered { components } => ConceptShape::Ordered(components.len()),
        }
    }

    fn ordered_components(&self) -> Option<&[ConceptId]> {
        match &self.kind {
            ConceptKind::Ordered { components } => Some(components),
            _ => None,
        }
    }

    fn coefficient_operand(&self) -> Option<(Relevance, &ConceptId)> {
        if !matches!(self.kind, ConceptKind::Unordered) || self.subconcepts.len() != 1 {
            return None;
        }
        let (concept, relevance) = self.subconcepts.first_key_value().unwrap();
        (*relevance != Relevance::DEFAULT).then_some((*relevance, concept))
    }

    fn children(&self) -> impl Iterator<Item = (&ConceptId, Relevance)> {
        let ordered = match &self.kind {
            ConceptKind::Ordered { components } => components.as_slice(),
            _ => &[],
        };

        ordered.iter().map(|child| (child, Relevance::DEFAULT)).chain(self.subconcepts.iter().map(|(child, &relevance)| (child, relevance)))
    }
}

/// A deterministic concept engine with isolated identity and percept state.
pub struct Pangine {
    id: usize,
    next_concept_id: Cell<usize>,
    names: BTreeMap<String, Weak<Concept>>,
    percepts: BTreeMap<String, ConceptId>,
    // Mutable Percepts use the same ConceptMap representation as ordinary
    // unordered Concept subconcepts. Keeping the map outside the Rc-backed
    // Concept avoids strong-reference cycles when Percepts contain Percepts.
    percept_subconcepts: BTreeMap<usize, ConceptMap>,
    // Recursive lookup postings retain complete source identities while
    // allowing questions to skip unrelated experiences.
    percept_question_indexes: BTreeMap<usize, PerceptQuestionIndex>,
    // Disposable materialization cache derived from the Percept subconcepts.
    percept_value_maps: BTreeMap<usize, ConceptMap>,
    // A linked output stores one live answer Concept here. Public value reads
    // return the projection cached inside that same Concept.
    percept_values: BTreeMap<usize, ConceptId>,
    // Percepts updated as replaceable current values are input/output Percepts
    // when experience is captured. Percepts populated by experience remain
    // references unless evaluation is explicitly requested with `$`.
    current_value_percepts: BTreeSet<usize>,
    composites: Vec<Weak<Concept>>,
    // Local accelerator for the existing weak canonical registry. Complete
    // equality, not the fingerprint, still determines Concept identity.
    composite_lookup: CompositeLookup,
    // Rebuild the weak indexes only as their stored size grows geometrically.
    next_index_prune_size: usize,
    #[cfg(test)]
    question_source_visits: usize,
}

impl Default for Pangine {
    fn default() -> Self {
        let id = NEXT_PANGINE_ID.fetch_add(1, AtomicOrdering::Relaxed);
        let global_percept = ConceptId::new(id, 0, ConceptKind::Percept { name: GLOBAL_PERCEPT_NAME.to_owned() }, ConceptMap::new());

        Self {
            id,
            next_concept_id: Cell::new(1),
            names: BTreeMap::new(),
            percepts: BTreeMap::from([(GLOBAL_PERCEPT_NAME.to_owned(), global_percept)]),
            percept_subconcepts: BTreeMap::new(),
            percept_question_indexes: BTreeMap::new(),
            percept_value_maps: BTreeMap::new(),
            percept_values: BTreeMap::new(),
            current_value_percepts: BTreeSet::new(),
            composites: Vec::new(),
            composite_lookup: CompositeLookup::new(),
            next_index_prune_size: 2,
            #[cfg(test)]
            question_source_visits: 0,
        }
    }
}

// Construction and direct composition.
impl Pangine {
    /// Creates an empty engine containing only the global percept.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the number of live ordinary concepts currently interned by this engine.
    pub fn concept_count(&self) -> usize {
        self.live_ordinary_concepts().count()
    }

    /// Returns the global percept handle.
    pub fn global_percept(&self) -> ConceptId {
        self.percepts[GLOBAL_PERCEPT_NAME].clone()
    }

    /// Composes complete Concept operands using the same unordered adjacency
    /// and coefficient normalization as parsed Pangine syntax.
    ///
    /// An empty slice produces no Concept. Nonempty operands can also normalize
    /// to no Concept when their coefficients cancel. One default-weight operand
    /// returns that operand directly. A nested multi-member unordered Concept
    /// remains one complete operand; this operation does not perform `*`
    /// merging.
    ///
    /// Returns [`ConceptConstructionError::ForeignConcept`] when an operand is
    /// not owned by this engine, or
    /// [`ConceptConstructionError::RelevanceOverflow`] when coefficient
    /// normalization exceeds the relevance range. Either error leaves the
    /// engine unchanged.
    pub fn compose_union(&mut self, operands: &[(Relevance, ConceptId)]) -> Result<Option<ConceptId>, ConceptConstructionError> {
        if operands.iter().any(|(_, concept)| !self.owns(concept)) {
            return Err(ConceptConstructionError::ForeignConcept);
        }

        let mut map = ConceptMap::new();
        for (relevance, concept) in operands {
            self.add_union_concept(&mut map, concept.clone(), false, *relevance).ok_or(ConceptConstructionError::RelevanceOverflow)?;
        }
        Ok(self.reference_map(&map))
    }

    /// Composes ordered component occurrences using the same canonical
    /// identity as an arrow chain in Pangine syntax.
    ///
    /// An empty slice produces no Concept, and a one-component composition
    /// returns that component directly. A supplied ordered Concept remains one
    /// complete component, matching a parenthesized operand in surface syntax.
    ///
    /// Returns [`ConceptConstructionError::ForeignConcept`] when a component is
    /// not owned by this engine. The error leaves the engine unchanged.
    pub fn compose_ordered(&mut self, components: &[ConceptId]) -> Result<Option<ConceptId>, ConceptConstructionError> {
        if components.iter().any(|concept| !self.owns(concept)) {
            return Err(ConceptConstructionError::ForeignConcept);
        }
        Ok((!components.is_empty()).then(|| self.reference_ordered(components.to_vec())))
    }
}

// Concept identity, state, and public mutation.
impl Pangine {
    /// Returns the stable named Concept for the exact UTF-8 text, creating it if necessary.
    ///
    /// Empty text is a named Concept and remains distinct from `[]`, which
    /// represents no Concept in Pangine syntax.
    pub fn reference_name(&mut self, name: &str) -> ConceptId {
        if let Some(concept) = self.names.get(name).and_then(Weak::upgrade) {
            return ConceptId(concept);
        }

        let concept = self.alloc(ConceptKind::Named(name.to_owned()), ConceptMap::new());
        self.names.insert(name.to_owned(), Rc::downgrade(&concept.0));
        self.maybe_prune_indexes();
        concept
    }

    /// Returns the stable percept handle for `name`, creating it if necessary.
    pub fn reference_percept(&mut self, name: &str) -> ConceptId {
        if let Some(concept) = self.percepts.get(name) {
            return concept.clone();
        }

        let concept = self.alloc(ConceptKind::Percept { name: name.to_owned() }, ConceptMap::new());
        self.percepts.insert(name.to_owned(), concept.clone());
        concept
    }

    /// Adds `addition` to a mutable Percept and returns its updated value.
    pub fn perform_addition(&mut self, percept: &ConceptId, addition: Option<&ConceptId>) -> Option<ConceptId> {
        self.perform_union_change(percept, addition, false)
    }

    /// Subtracts `subtraction` from a mutable Percept and returns its updated value.
    pub fn perform_subtraction(&mut self, percept: &ConceptId, subtraction: Option<&ConceptId>) -> Option<ConceptId> {
        self.perform_union_change(percept, subtraction, true)
    }

    /// Explicitly merges `merge` into a mutable percept and returns its updated value.
    pub fn perform_merge(&mut self, percept: &ConceptId, merge: Option<&ConceptId>) -> Option<ConceptId> {
        self.perform_merge_change(percept, merge, false)
    }

    /// Explicitly merges the inverse of `merge` into a mutable percept and returns its updated value.
    pub fn perform_inverse_merge(&mut self, percept: &ConceptId, merge: Option<&ConceptId>) -> Option<ConceptId> {
        self.perform_merge_change(percept, merge, true)
    }

    /// Evaluates a complete Concept and adds default relevance to the grounded result under a mutable Percept.
    ///
    /// Every nested Percept holding a replaceable current value is evaluated
    /// before the experience is retained. Percepts populated by experience
    /// remain references unless they are explicitly evaluated with
    /// [`Self::evaluate_concept`]. A missing required input value produces no
    /// experience and leaves the target unchanged.
    pub fn perform_experience(&mut self, percept: &ConceptId, experience: Option<&ConceptId>) -> Option<ConceptId> {
        if !self.accepts_percept_input(percept, experience) {
            return None;
        }

        let Some(experience) = experience else {
            return self.get_value(percept);
        };

        let experience = self.evaluate_experience_concept(experience)?;
        if self.live_answer_value(percept).is_some() {
            let projection = self.get_value(percept);
            self.set_percept_values(&[(percept.clone(), projection)]).ok()?;
        }
        self.record_experience(percept, &experience)?;
        self.current_value_percepts.remove(&percept.index());
        self.materialize_percept_value(percept)
    }

    /// Returns a concept's kind when it belongs to this engine.
    pub fn concept_kind<'a>(&self, concept: &'a ConceptId) -> Option<&'a ConceptKind> {
        self.owns(concept).then_some(&concept.0.kind)
    }

    /// Returns the name of an owned named concept.
    pub fn get_name<'a>(&self, concept: &'a ConceptId) -> Option<&'a str> {
        match self.concept_kind(concept)? {
            ConceptKind::Named(name) => Some(name.as_str()),
            _ => None,
        }
    }

    /// Returns the current visible value of an owned Percept.
    ///
    /// A linked question output returns its projection from the ordinary answer
    /// Concept shared by that answer's outputs.
    pub fn get_value(&self, concept: &ConceptId) -> Option<ConceptId> {
        if !self.is_percept(concept) {
            return None;
        }

        if self.is_global_percept(concept) {
            return self.global_value();
        }

        let value = self.percept_values.get(&concept.index())?.clone();
        if let Some((_, live)) = self.live_answer_value(concept) {
            if let Some(projection) = live.projection(concept) {
                return projection;
            }
        }
        Some(value)
    }

    /// Replaces a mutable Percept's current value, returning whether the input was valid.
    ///
    /// A Percept set through this operation is evaluated automatically when it
    /// appears inside a later experience. Assigning a question output also
    /// detaches it from that question's shared answer.
    pub fn set_percept_value(&mut self, percept: &ConceptId, value: Option<ConceptId>) -> bool {
        self.set_percept_values(&[(percept.clone(), value)]).is_ok()
    }

    /// Replaces several mutable Percept values as one validated update.
    ///
    /// Every supplied Percept and value is checked before any value changes.
    /// The same Percept cannot appear twice. An error therefore leaves the
    /// complete group unchanged. Each updated Percept is evaluated automatically
    /// when it appears inside a later experience.
    pub fn set_percept_values(&mut self, updates: &[(ConceptId, Option<ConceptId>)]) -> Result<(), PerceptUpdateError> {
        let mut supplied_percepts = BTreeSet::new();
        for (percept, value) in updates {
            if !self.is_mutable_percept(percept) {
                return Err(PerceptUpdateError::InvalidPercept);
            }
            if value.as_ref().is_some_and(|concept| !self.owns(concept)) {
                return Err(PerceptUpdateError::ForeignConcept);
            }
            if !supplied_percepts.insert(percept.clone()) {
                return Err(PerceptUpdateError::DuplicatePercept);
            }
        }

        let mut detached = Vec::new();
        let mut visited_values = BTreeSet::new();
        for percept in &supplied_percepts {
            let Some((answer_value, live)) = self.live_answer_value(percept) else {
                continue;
            };
            if !visited_values.insert(answer_value.clone()) {
                continue;
            }

            let outputs = live.answer.outputs.clone();
            let mut answer = Some(live.answer);
            for output in outputs.intersection(&supplied_percepts) {
                answer = answer.and_then(|answer| answer.detach(self, output));
            }
            if let Some(answer) = answer {
                detached.push(LiveConceptAnswer::successor(self, live.revision, answer).ok_or(PerceptUpdateError::InvalidPercept)?);
            }
        }

        for live in detached {
            self.install_live_answer(live).ok_or(PerceptUpdateError::InvalidPercept)?;
        }
        for (percept, value) in updates {
            self.write_current_percept_value(percept, value.clone());
        }
        Ok(())
    }

    /// Returns an owned ordered composition's component occurrences.
    pub fn get_ordered_components(&self, concept: &ConceptId) -> Option<Vec<ConceptId>> {
        let ConceptKind::Ordered { components } = self.concept_kind(concept)? else {
            return None;
        };
        Some(components.clone())
    }

    /// Returns entries ordered by descending `x`, then canonical Concept order.
    ///
    /// A Percept returns its direct represented Concepts. The read-only global
    /// Percept computes that set from every live ordinary Concept. An unordered
    /// composition returns its member edges. Any other Concept is treated as a
    /// single default-coefficient entry.
    pub fn get_relevance_map(&self, concept: &ConceptId) -> Vec<(Relevance, ConceptId)> {
        let map = if self.is_global_percept(concept) {
            self.global_concept_map().into_iter().map(|(concept, relevance)| (relevance, concept)).collect()
        } else if self.is_mutable_percept(concept) {
            if self.live_answer_value(concept).is_some() {
                self.get_value(concept).into_iter().map(|value| (Relevance::DEFAULT, value)).collect()
            } else {
                self.percept_subconcepts.get(&concept.index()).into_iter().flatten().map(|(concept, &relevance)| (relevance, concept.clone())).collect()
            }
        } else {
            self.relevance_entries(concept).unwrap_or_default()
        };
        self.sorted_relevance_entries(map)
    }

    fn percept_value_map(&mut self, percept: &ConceptId) -> Option<ConceptMap> {
        if !self.is_percept(percept) {
            return None;
        }

        let mut map = ConceptMap::new();
        if let Some(current) = self.get_value(percept) {
            self.add_merge_concept(&mut map, current, false, Relevance::DEFAULT)?;
        }
        Some(map)
    }

    fn perform_union_change(&mut self, percept: &ConceptId, concept: Option<&ConceptId>, inversion: bool) -> Option<ConceptId> {
        if !self.accepts_percept_input(percept, concept) {
            return None;
        }
        let mut map = self.percept_union_value_map(percept)?;
        if let Some(concept) = concept {
            self.add_union_concept(&mut map, concept.clone(), inversion, Relevance::DEFAULT)?;
        }

        let value = self.reference_map(&map);
        self.set_percept_value(percept, value.clone());
        value
    }

    fn perform_merge_change(&mut self, percept: &ConceptId, concept: Option<&ConceptId>, inversion: bool) -> Option<ConceptId> {
        if !self.accepts_percept_input(percept, concept) {
            return None;
        }
        let value = self.percept_value_map(percept).and_then(|mut map| {
            if let Some(concept) = concept {
                self.add_merge_concept(&mut map, concept.clone(), inversion, Relevance::DEFAULT)?;
            }
            self.reference_map(&map)
        });
        self.set_percept_value(percept, value.clone());
        value
    }

    fn percept_union_value_map(&mut self, percept: &ConceptId) -> Option<ConceptMap> {
        if !self.is_percept(percept) {
            return None;
        }

        let Some(current) = self.get_value(percept) else {
            return Some(ConceptMap::new());
        };

        if matches!(current.0.kind, ConceptKind::Unordered) {
            return Some(current.0.subconcepts.clone());
        }

        let mut map = ConceptMap::new();
        self.add_union_concept(&mut map, current, false, Relevance::DEFAULT)?;
        Some(map)
    }

    fn sorted_relevance_entries(&self, mut entries: Vec<(Relevance, ConceptId)>) -> Vec<(Relevance, ConceptId)> {
        entries.sort_by(|(left_rel, left_concept), (right_rel, right_concept)| {
            compare_coefficients_desc(*left_rel, *right_rel).then_with(|| self.compare_concepts(left_concept, right_concept))
        });
        entries
    }

    fn relevance_entries(&self, concept: &ConceptId) -> Option<Vec<(Relevance, ConceptId)>> {
        if !self.owns(concept) {
            return None;
        }

        Some(if matches!(concept.0.kind, ConceptKind::Unordered) {
            concept.0.subconcepts.iter().map(|(concept, &relevance)| (relevance, concept.clone())).collect()
        } else {
            vec![(Relevance::DEFAULT, concept.clone())]
        })
    }
}

struct ParsedUnionOperand {
    concept: ConceptId,
    relevance: Relevance,
}

impl ParsedUnionOperand {
    fn ordinary(concept: ConceptId) -> Self {
        Self { concept, relevance: Relevance::DEFAULT }
    }
}

fn compare_coefficients_desc(left: Relevance, right: Relevance) -> Ordering {
    right.count().cmp(&left.count())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dropping_pangine_releases_percept_value_graphs() {
        let weak_value = {
            let mut pangine = Pangine::new();
            let percept = pangine.reference_percept("memory");
            let value = pangine.reference_concept("{memory}[A]").unwrap().unwrap();
            let weak_value = Rc::downgrade(&value.0);

            assert!(pangine.set_percept_value(&percept, Some(value.clone())));
            drop(value);
            assert!(weak_value.upgrade().is_some());
            weak_value
        };

        assert!(weak_value.upgrade().is_none());
    }

    #[test]
    fn ordinary_answer_values_are_replaced_and_released_during_detachment() {
        let mut pangine = Pangine::new();
        pangine.reference_concept("{memory} ~= [cat]->[purrs]").unwrap();
        pangine.reference_concept("{memory} @ {animal}->{sound}").unwrap();
        let animal = pangine.reference_percept("animal");
        let sound = pangine.reference_percept("sound");
        let initial = pangine.percept_values[&animal.index()].clone();
        assert_eq!(pangine.percept_values[&sound.index()], initial);
        let weak_initial = Rc::downgrade(&initial.0);
        drop(initial);

        pangine.reference_concept("{animal} = []").unwrap();
        assert!(weak_initial.upgrade().is_none());
        let remaining = pangine.percept_values[&sound.index()].clone();
        let weak_remaining = Rc::downgrade(&remaining.0);
        drop(remaining);

        pangine.reference_concept("{sound} = []").unwrap();
        assert!(weak_remaining.upgrade().is_none());
    }

    #[test]
    fn answer_extension_is_atomic_when_a_projection_overflows() {
        let mut pangine = Pangine::new();
        let meals = pangine.reference_percept("meals");
        let meal = pangine.reference_concept("[cat]->[eats]->[fish]").unwrap().unwrap();
        assert!(pangine.set_percept_subconcepts(&meals, ConceptMap::from([(meal, Relevance::new(i64::MAX))])).is_some());
        pangine.reference_concept("{meals} @ {animal}->[eats]->{food}").unwrap().unwrap();
        pangine.reference_concept("{home} = [old-home]").unwrap().unwrap();
        let homes = pangine.reference_percept("homes");
        let home = pangine.reference_concept("[cat]->[lives-in]->[house]").unwrap().unwrap();
        assert!(pangine.set_percept_subconcepts(&homes, ConceptMap::from([(home, Relevance::new(2))])).is_some());

        let linked_before = pangine.reference_concept("&{animal}").unwrap().unwrap();
        let animal_before = pangine.reference_concept("${animal}").unwrap().unwrap();
        assert!(pangine.reference_concept("{homes} @ {animal}->[lives-in]->{home}").unwrap().is_none());

        assert_eq!(pangine.reference_concept("&{animal}").unwrap(), Some(linked_before));
        assert_eq!(pangine.reference_concept("${animal}").unwrap(), Some(animal_before));
        let old_home = pangine.reference_concept("[old-home]").unwrap();
        assert_eq!(pangine.reference_concept("${home}").unwrap(), old_home);
        assert!(pangine.reference_concept("&{home}").unwrap().is_none());
    }

    #[test]
    fn composite_lookup_reuses_equal_full_width_integer_coefficients() {
        let mut pangine = Pangine::new();
        let member = pangine.reference_named("member").unwrap();
        let first_map = ConceptMap::from([(member.clone(), Relevance::new(i64::MAX))]);
        let second_map = ConceptMap::from([(member, Relevance::new(i64::MAX))]);

        assert_eq!(first_map, second_map);
        assert_eq!(Pangine::composite_fingerprint(&ConceptKind::Unordered, &first_map), Pangine::composite_fingerprint(&ConceptKind::Unordered, &second_map));
        let first = pangine.reference_composite(ConceptKind::Unordered, first_map);
        let second = pangine.reference_composite(ConceptKind::Unordered, second_map);
        assert_eq!(first, second);
    }

    #[test]
    fn incremental_experience_materialization_matches_a_full_subconcept_rebuild() {
        let mut pangine = Pangine::new();
        let percept = pangine.reference_percept("memory");
        let atomic = pangine.reference_concept("[A]").unwrap().unwrap();
        let inverse = pangine.reference_concept("![A]").unwrap().unwrap();
        let pair = pangine.reference_concept("[A][B]").unwrap().unwrap();
        let coefficient_pair = pangine.reference_concept("x2[A][B]").unwrap().unwrap();
        let sequence = [coefficient_pair.clone(), atomic, pair, inverse, coefficient_pair];

        for (step, concept) in sequence.into_iter().enumerate() {
            let concept_text = pangine.format_concept(&concept, false);
            let value = pangine.perform_experience(&percept, Some(&concept));
            assert_eq!(value, pangine.get_value(&percept));

            let subconcepts = pangine.percept_subconcepts[&percept.index()].clone();
            let rebuilt = pangine.materialized_percept_map(&subconcepts).unwrap();
            assert_eq!(pangine.percept_value_maps[&percept.index()], rebuilt, "after step {step} experiencing {concept_text}");
        }

        let previous = pangine.get_value(&percept).unwrap();
        let previous_text = pangine.format_concept(&previous, false);
        pangine.percept_value_maps.remove(&percept.index());
        let final_concept = pangine.reference_concept("[C]->[D]").unwrap().unwrap();
        pangine.perform_experience(&percept, Some(&final_concept));
        let subconcepts = pangine.percept_subconcepts[&percept.index()].clone();
        let rebuilt = pangine.materialized_percept_map(&subconcepts).unwrap();
        assert_eq!(pangine.percept_value_maps[&percept.index()], rebuilt);
        assert_eq!(pangine.format_concept(&previous, false), previous_text);

        let current_text = pangine.format_concept(&pangine.get_value(&percept).unwrap(), false);
        pangine.percept_value_maps.remove(&percept.index());
        pangine.percept_values.remove(&percept.index());
        let restored = pangine.materialize_percept_value(&percept).unwrap();
        assert_eq!(pangine.format_concept(&restored, false), current_text);
    }

    #[test]
    fn every_retained_experience_return_keeps_its_original_value_and_identity() {
        let mut pangine = Pangine::new();
        let percept = pangine.reference_percept("memory");
        let concepts = (0..64)
            .map(|index| {
                let item = pangine.reference_named(&format!("item-{index}")).unwrap();
                let answer = pangine.reference_named(&format!("answer-{index}")).unwrap();
                pangine.reference_ordered(vec![item, answer])
            })
            .collect::<Vec<_>>();
        let mut returns = Vec::with_capacity(concepts.len());

        for concept in &concepts {
            pangine.record_experience(&percept, concept).unwrap();
            returns.push(pangine.materialize_percept_value(&percept).unwrap());
        }

        for (index, returned) in returns.iter().enumerate() {
            let expected = concepts[..=index].iter().cloned().map(|concept| (concept, Relevance::DEFAULT)).collect::<ConceptMap>();
            let reconstructed = pangine.reference_map(&expected).unwrap();
            assert_eq!(&reconstructed, returned);
            assert_eq!(pangine.format_concept(&reconstructed, false), pangine.format_concept(returned, false));
        }
    }
}

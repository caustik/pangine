use crate::Relevance;
use std::cell::Cell;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::{self, Write};
use std::path::Path;
use std::rc::{Rc, Weak};
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

type ConceptMap = BTreeMap<ConceptId, Relevance>;
type ExperienceRoots = BTreeMap<ConceptId, u64>;
type ProjectionAssignment = BTreeMap<ConceptId, ConceptId>;
type ProjectionAssignments = BTreeSet<ProjectionAssignment>;
type ProjectionCache = BTreeMap<(usize, usize), ProjectionAssignments>;
// One exact experienced root supports a candidate according to that root's
// occurrence count. Recursive matches and alternate routes to the same answer
// within that root collapse here.
type QuestionCandidateWitnesses = BTreeMap<ConceptId, BTreeMap<ConceptId, BTreeSet<QuestionSource>>>;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ConceptShape {
    Named,
    Percept,
    Unordered,
    Ordered(usize),
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct QuestionSource {
    percept: ConceptId,
    root: ConceptId,
    occurrences: u64,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct QuestionSeed {
    source: QuestionSource,
    concept: ConceptId,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
enum QuestionPort {
    Ordered(usize),
    Unordered(ConceptId),
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct QuestionMembership {
    parent: ConceptId,
    port: QuestionPort,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct QuestionExperience {
    source: QuestionSource,
    matched: ConceptId,
    blocked_memberships: BTreeSet<QuestionMembership>,
}

#[derive(Clone)]
struct QuestionStep {
    destination: ConceptId,
    source: QuestionSource,
    membership: QuestionMembership,
}

#[derive(Default)]
struct QuestionGraph {
    steps: BTreeMap<ConceptId, Vec<QuestionStep>>,
}

impl QuestionGraph {
    fn add_membership(&mut self, source: &QuestionSource, parent: &ConceptId, child: &ConceptId, port: QuestionPort) {
        if parent == child {
            return;
        }

        let membership = QuestionMembership { parent: parent.clone(), port };
        self.steps.entry(parent.clone()).or_default().push(QuestionStep { destination: child.clone(), source: source.clone(), membership: membership.clone() });
        self.steps.entry(child.clone()).or_default().push(QuestionStep { destination: parent.clone(), source: source.clone(), membership });
    }

    fn connects(&self, start: &ConceptId, target: &ConceptId, answer: &QuestionExperience) -> bool {
        if start == target {
            return true;
        }

        let mut frontier = BTreeSet::from([start.clone()]);
        let mut visited = frontier.clone();
        while !frontier.is_empty() {
            let mut next = BTreeSet::new();
            for concept in frontier {
                for step in self.steps.get(&concept).into_iter().flatten() {
                    if step.source == answer.source && answer.blocked_memberships.contains(&step.membership) {
                        continue;
                    }
                    if step.destination == *target {
                        return true;
                    }
                    if visited.insert(step.destination.clone()) {
                        next.insert(step.destination.clone());
                    }
                }
            }
            frontier = next;
        }
        false
    }
}

#[derive(Default)]
struct QuestionSnapshot {
    experiences: BTreeSet<QuestionExperience>,
    source_concepts: BTreeSet<QuestionSeed>,
    graph: QuestionGraph,
}

static NEXT_PANGINE_ID: AtomicUsize = AtomicUsize::new(0);

/// The reserved name of the global percept.
pub const GLOBAL_PERCEPT_NAME: &str = "*";

const DEBUG_CONSOLE_HELP: &str = "\
Commands:
  help, h        Show this help
  quit, q        Exit

Concept syntax:
  []                         Null / no concept
  [name]                     Named concept
  ['name']                   Percept reference
  (expression)               Grouping
  [A][B]                     Union
  [A]*[B]                    Flattening merge
  [A]/[B]                    Merge with inverted [B]
  ![A]                       Inversion
  [A]->[B]->[C]              Ordered composition
  50%x2[A]x3[B]              Relevance

Percept operations:
  ['name'] = expression      Assign
  ['name'] += expression     Union addition
  ['name'] -= expression     Union subtraction
  ['name'] *= expression     Flattening merge
  ['name'] /= expression     Inverse merge
  ['name'] ~= expression     Experience
  ['source'] @ expression    Ask one Percept; bind outputs in the question
  ['a']['b'] @ expression   Ask several Percepts together
  $operand                   Recursively evaluate every percept in the operand
  $['*']                     Inspect all live ordinary concepts

Experience:
  ['memory'] ~= {[cat]->[purrs]}
  Records the complete input as one experience owned by ['memory']. Repeating
  an equal root increments its occurrence count. Questions derive recursive
  matches from exact roots without multiplying one experience by match routes.

Scripts:
  expression; expression    Multiple statements
  // line comment            C++-style comment
  /* block comment */        C-style comment

Choice:
  ^['choice'] evaluates the percept and greedily returns the entry with the
  greatest positive relevance weight.

  ['choice'] = x2[tea]x3[coffee]
  ^['choice']             returns [coffee]

  Exact top-weight ties use the earliest canonical Concept spelling. If no
  entry has positive weight, ^ returns []. Zero-weight entries disappear when
  their Concept is built and are not decision candidates. This is a
  deterministic greedy rule, not a probability model or a random sampler.
";

/// The result of parsing or executing Pangine syntax.
pub type ParseResult<T> = Result<T, ParseError>;

/// An error produced while parsing a script or reading a script file.
#[derive(Debug)]
#[non_exhaustive]
pub enum ParseError {
    /// The input does not conform to Pangine syntax.
    InvalidSyntax,
    /// A script or details file could not be read or written.
    Io(io::Error),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidSyntax => formatter.write_str("invalid Pangine syntax"),
            Self::Io(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ParseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidSyntax => None,
            Self::Io(error) => Some(error),
        }
    }
}

impl From<io::Error> for ParseError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

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
    /// An unordered composition whose member edges carry Relevance.
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
    percept_roots: BTreeMap<usize, ExperienceRoots>,
    // Disposable materialization cache derived from the exact roots.
    percept_value_maps: BTreeMap<usize, ConceptMap>,
    percept_values: BTreeMap<usize, ConceptId>,
    composites: Vec<Weak<Concept>>,
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
            percept_roots: BTreeMap::new(),
            percept_value_maps: BTreeMap::new(),
            percept_values: BTreeMap::new(),
            composites: Vec::new(),
        }
    }
}

// Construction and script entry points.
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

    /// Parses and executes a Pangine statement or expression.
    pub fn reference_concept(&mut self, script: &str) -> ParseResult<Option<ConceptId>> {
        let result = self.parse_statement_text(script);
        self.prune_indexes();
        result
    }

    /// Parses and executes every statement in a script string.
    pub fn parse_script_text(&mut self, script: &str) -> ParseResult<Option<ConceptId>> {
        let result = self.parse_script_text_impl(script, None);
        self.prune_indexes();
        result
    }

    /// Parses a script string while writing each statement and result to `details`.
    pub fn parse_script_text_with_details<W: Write>(&mut self, script: &str, details: &mut W) -> ParseResult<Option<ConceptId>> {
        let result = self.parse_script_text_impl(script, Some(details));
        self.prune_indexes();
        result
    }

    /// Reads, parses, and executes a UTF-8 script file.
    pub fn parse_script_file(&mut self, path: impl AsRef<Path>) -> ParseResult<Option<ConceptId>> {
        let script = fs::read_to_string(path)?;
        self.parse_script_text(&script)
    }

    /// Parses a script file while writing each statement and result to `details`.
    pub fn parse_script_file_with_details<W: Write>(&mut self, path: impl AsRef<Path>, details: &mut W) -> ParseResult<Option<ConceptId>> {
        let script = fs::read_to_string(path)?;
        self.parse_script_text_with_details(&script, details)
    }

    /// Parses a script file and writes execution details to another file.
    pub fn parse_script_file_to_details_file(&mut self, path: impl AsRef<Path>, details_path: impl AsRef<Path>) -> ParseResult<Option<ConceptId>> {
        let mut details = fs::File::create(details_path)?;
        self.parse_script_file_with_details(path, &mut details)
    }

    fn parse_script_text_impl(&mut self, script: &str, mut details: Option<&mut dyn Write>) -> ParseResult<Option<ConceptId>> {
        let mut result = None;
        let statements = split_script_statements(script);

        for statement in statements.items {
            if !statement_has_tokens(statement) {
                continue;
            }

            if let Some(details) = details.as_mut() {
                writeln!(&mut **details, "ps> {statement}")?;
            }

            let concept = match self.parse_statement_text(statement) {
                Ok(concept) => concept,
                Err(error) => {
                    if let Some(details) = details.as_mut() {
                        writeln!(&mut **details, "ps!   {error}")?;
                    }
                    return Err(error);
                }
            };

            if let Some(details) = details.as_mut() {
                let formatted = concept.as_ref().map_or_else(|| "[]".to_owned(), |concept| self.format_concept(concept, false));
                writeln!(&mut **details, "ps=   {formatted}")?;
            }

            result = if statements.has_semicolons { concept } else { concept.or(result) };
        }

        Ok(result)
    }
}

// Concept identity, state, and public mutation.
impl Pangine {
    /// Returns the stable percept handle for `name`, creating it if necessary.
    pub fn reference_percept(&mut self, name: &str) -> ConceptId {
        if let Some(concept) = self.percepts.get(name) {
            return concept.clone();
        }

        let concept = self.alloc(ConceptKind::Percept { name: name.to_owned() }, ConceptMap::new());
        self.percepts.insert(name.to_owned(), concept.clone());
        concept
    }

    /// Adds `addition` to a mutable percept and returns its updated value.
    pub fn perform_addition(&mut self, percept: &ConceptId, addition: Option<&ConceptId>) -> Option<ConceptId> {
        if !self.accepts_percept_input(percept, addition) {
            return None;
        }

        let (value, stored) = self.perform_union_update(percept, addition.cloned(), false)?;
        self.set_percept_value(percept, stored);
        value
    }

    /// Subtracts `subtraction` from a mutable percept and returns its updated value.
    pub fn perform_subtraction(&mut self, percept: &ConceptId, subtraction: Option<&ConceptId>) -> Option<ConceptId> {
        if !self.accepts_percept_input(percept, subtraction) {
            return None;
        }

        let (value, stored) = self.perform_union_update(percept, subtraction.cloned(), true)?;
        self.set_percept_value(percept, stored);
        value
    }

    /// Flattens `merge` into a mutable percept and returns its updated value.
    pub fn perform_merge(&mut self, percept: &ConceptId, merge: Option<&ConceptId>) -> Option<ConceptId> {
        if !self.accepts_percept_input(percept, merge) {
            return None;
        }

        let value = self.perform_merge_update(percept, merge.cloned(), false);
        self.set_percept_value(percept, value.clone());
        value
    }

    /// Flattens the inverse of `merge` into a mutable percept and returns its updated value.
    pub fn perform_inverse_merge(&mut self, percept: &ConceptId, merge: Option<&ConceptId>) -> Option<ConceptId> {
        if !self.accepts_percept_input(percept, merge) {
            return None;
        }

        let value = self.perform_merge_update(percept, merge.cloned(), true);
        self.set_percept_value(percept, value.clone());
        value
    }

    /// Records one occurrence of an exact complete root under a mutable Percept.
    pub fn perform_experience(&mut self, percept: &ConceptId, experience: Option<&ConceptId>) -> Option<ConceptId> {
        if !self.accepts_percept_input(percept, experience) {
            return None;
        }

        let Some(experience) = experience else {
            return self.get_value(percept);
        };

        self.record_experience(percept, experience)?;
        self.materialize_percept_value(percept)
    }

    /// Returns a concept's kind when it belongs to this engine.
    pub fn concept_kind<'a>(&self, concept: &'a ConceptId) -> Option<&'a ConceptKind> {
        self.owns(concept).then_some(&concept.0.kind)
    }

    /// Returns the name of an owned named concept.
    pub fn get_name<'a>(&self, concept: &'a ConceptId) -> Option<&'a str> {
        if !self.owns(concept) {
            return None;
        }

        match &concept.0.kind {
            ConceptKind::Named(name) => Some(name.as_str()),
            _ => None,
        }
    }

    /// Returns the current value of an owned percept.
    pub fn get_value(&self, concept: &ConceptId) -> Option<ConceptId> {
        if !self.is_percept(concept) {
            return None;
        }

        if self.is_global_percept(concept) {
            return self.global_value();
        }

        self.percept_values.get(&concept.index()).cloned()
    }

    /// Returns a mutable Percept's unique exact experience roots in canonical order.
    pub fn get_percept_roots(&self, percept: &ConceptId) -> Option<Vec<ConceptId>> {
        if !self.is_mutable_percept(percept) {
            return None;
        }

        let mut roots = self.percept_roots.get(&percept.index()).into_iter().flatten().map(|(root, _)| root.clone()).collect::<Vec<_>>();
        roots.sort_by(|left, right| self.compare_concepts(left, right));
        Some(roots)
    }

    /// Returns how many times an exact root was experienced under a mutable Percept.
    pub fn get_percept_root_count(&self, percept: &ConceptId, root: &ConceptId) -> Option<u64> {
        if !self.is_mutable_percept(percept) || !self.owns(root) {
            return None;
        }

        Some(self.percept_roots.get(&percept.index()).and_then(|roots| roots.get(root)).copied().unwrap_or_default())
    }

    /// Replaces a mutable percept's value, returning whether the input was valid.
    pub fn set_percept_value(&mut self, percept: &ConceptId, value: Option<ConceptId>) -> bool {
        if !self.is_mutable_percept(percept) || value.as_ref().is_some_and(|concept| !self.owns(concept)) {
            return false;
        }

        let roots = value.into_iter().map(|root| (root, 1)).collect();
        self.set_percept_roots(percept, roots);
        true
    }

    /// Returns an owned ordered composition's component occurrences.
    pub fn get_ordered_components(&self, concept: &ConceptId) -> Option<Vec<ConceptId>> {
        if !self.owns(concept) {
            return None;
        }

        concept.0.ordered_components().map(<[ConceptId]>::to_vec)
    }

    /// Returns `concept` when it is an owned percept.
    pub fn get_percept(&self, concept: &ConceptId) -> Option<ConceptId> {
        self.is_percept(concept).then(|| concept.clone())
    }

    /// Returns relevance entries ordered by descending weight and Concept identity.
    ///
    /// An unordered composition returns its member edges. Any other Concept is
    /// treated as a single default-relevance entry.
    pub fn get_relevance_map(&self, concept: &ConceptId) -> Vec<(Relevance, ConceptId)> {
        let mut map = self.relevance_entries(concept).unwrap_or_default();

        map.sort_by(|(left_rel, left_concept), (right_rel, right_concept)| {
            compare_relevance_desc(*left_rel, *right_rel).then_with(|| left_concept.cmp(right_concept))
        });
        map
    }
}

// Canonical presentation and the interactive console.
impl Pangine {
    /// Formats relevance entries as individual debug-console lines.
    pub fn debug_console_lines(&self, concept: Option<&ConceptId>) -> Vec<String> {
        // Historical anchor:
        // 1.x/pangine/src/pangine/common/pae_pangine.cpp:1311
        let Some(entries) = concept.and_then(|concept| self.relevance_entries(concept)) else {
            return vec!["  []".to_owned()];
        };

        entries.into_iter().map(|(relevance, concept)| self.format_debug_console_line(relevance, &concept)).collect()
    }

    /// Formats an owned concept as canonical Pangine syntax.
    pub fn format_concept(&self, concept: &ConceptId, evaluate: bool) -> String {
        if !self.owns(concept) {
            return "[]".to_owned();
        }

        let mut active = BTreeSet::new();
        self.format_inner(concept, evaluate, &mut active)
    }

    /// Formats a concept, optionally evaluating percept references recursively.
    pub fn recurse(&self, concept: &ConceptId, evaluate: bool) -> String {
        self.format_concept(concept, evaluate)
    }

    /// Runs the interactive Pangine console on standard input and output.
    pub fn debug_console(&mut self) -> io::Result<()> {
        let stdin = io::stdin();
        let mut input = String::new();

        loop {
            print!("command> ");
            io::stdout().flush()?;

            input.clear();
            if stdin.read_line(&mut input)? == 0 {
                break;
            }

            let script = input.trim_end_matches(['\r', '\n']);

            if debug_console_quit(script) {
                break;
            }

            if let Some(help) = debug_console_help(script) {
                print!("{help}");
                continue;
            }

            match self.reference_concept(script) {
                Ok(concept) => {
                    for line in self.debug_console_lines(concept.as_ref()) {
                        println!("{line}");
                    }
                }
                Err(error) => println!("  {error}"),
            }
        }

        Ok(())
    }
}

// Recursive-descent parser implementation.
impl Pangine {
    fn parse_expression(&mut self, parser: &mut Parser) -> ParseResult<Option<ConceptId>> {
        let selector = self.parse_ordered_expression(parser)?;

        parser.skip_ws();
        if !parser.consume('@') {
            return Ok(selector);
        }

        let selector = selector.ok_or(ParseError::InvalidSyntax)?;
        let percepts = self.question_percepts(&selector).ok_or(ParseError::InvalidSyntax)?;
        parser.skip_ws();
        let question_start = parser.pos;
        let question = self.parse_expression(parser)?;
        if parser.pos == question_start {
            return Err(ParseError::InvalidSyntax);
        }
        Ok(self.answer_question(&percepts, question))
    }

    // An unparenthesized arrow chain is one ordered composition. Parentheses
    // can still place a complete ordered composition in one component.
    fn parse_ordered_expression(&mut self, parser: &mut Parser) -> ParseResult<Option<ConceptId>> {
        let Some(first) = self.parse_merge_expression(parser)? else {
            return Ok(None);
        };
        let mut components = vec![first];

        loop {
            parser.skip_ws();
            if !parser.consume_str("->") {
                break;
            }

            let component = self.parse_merge_expression(parser)?.ok_or(ParseError::InvalidSyntax)?;
            components.push(component);
        }

        Ok(Some(self.reference_ordered(components)))
    }

    fn parse_merge_expression(&mut self, parser: &mut Parser) -> ParseResult<Option<ConceptId>> {
        let mut concept = self.parse_union(parser)?;

        loop {
            parser.skip_ws();
            let inversion = if parser.consume('*') {
                false
            } else if parser.consume('/') {
                true
            } else {
                return Ok(concept);
            };

            if concept.is_none() {
                return Err(ParseError::InvalidSyntax);
            }

            parser.skip_ws();
            let rhs_start = parser.pos;
            let rhs = self.parse_union(parser)?;
            if rhs.is_none() && parser.pos == rhs_start {
                return Err(ParseError::InvalidSyntax);
            }
            if rhs.is_none() {
                return Ok(None);
            }
            concept = self.reference_merge_with_inversion(concept, rhs, inversion);
        }
    }

    fn parse_statements(&mut self, parser: &mut Parser) -> ParseResult<Option<ConceptId>> {
        let mut result = None;

        loop {
            parser.skip_ws();
            if parser.peek().is_none() {
                return Ok(result);
            }

            result = self.parse_expression(parser)?;
            parser.skip_ws();
            if !parser.consume(';') {
                return Ok(result);
            }
        }
    }

    fn parse_statement_text(&mut self, script: &str) -> ParseResult<Option<ConceptId>> {
        let mut parser = Parser::new(script);
        let concept = self.parse_statements(&mut parser)?;
        parser.skip_ws();
        parser.peek().is_none().then_some(concept).ok_or(ParseError::InvalidSyntax)
    }

    fn parse_union(&mut self, parser: &mut Parser) -> ParseResult<Option<ConceptId>> {
        let mut concepts = Vec::new();

        if let Some(concept) = self.parse_union_operand(parser)? {
            concepts.push(concept);
        }

        loop {
            parser.skip_ws();
            if !parser.starts_union_operand() {
                break;
            }

            if let Some(concept) = self.parse_union_operand(parser)? {
                concepts.push(concept);
            }
        }

        Ok(self.reference_union(&concepts))
    }

    fn parse_union_operand(&mut self, parser: &mut Parser) -> ParseResult<Option<ConceptId>> {
        parser.skip_ws();

        let relevance_start = parser.pos;
        let probability = parser.parse_probability();
        let strength = if parser.consume('x') { parser.parse_float() } else { 1.0 };

        if parser.pos != relevance_start {
            let term = self.parse_union_operand(parser)?.ok_or(ParseError::InvalidSyntax)?;
            let mut map = ConceptMap::new();
            self.add_union_concept(&mut map, term, false, Relevance::new(probability, strength));
            return Ok(self.reference_map(&map));
        }

        match parser.peek() {
            Some('(') => {
                parser.next();
                let concept = self.parse_expression(parser)?;
                parser.expect(')')?;
                Ok(concept)
            }
            Some('[') => self.parse_bracket(parser),
            Some('{') => self.parse_ordered(parser),
            Some('$') => {
                parser.next();
                let evaluated = self.parse_union_operand(parser)?.ok_or(ParseError::InvalidSyntax)?;
                Ok(self.evaluate_concept(&evaluated))
            }
            Some('^') => {
                parser.next();
                let decision = self.parse_union_operand(parser)?.ok_or(ParseError::InvalidSyntax)?;
                Ok(self.make_decision(&decision))
            }
            Some('!') => {
                parser.next();
                parser.skip_ws();
                let concept_start = parser.pos;
                let concept = self.parse_union_operand(parser)?;
                if concept.is_none() && parser.pos == concept_start {
                    return Err(ParseError::InvalidSyntax);
                }
                Ok(self.reference_inversion(concept))
            }
            _ => Ok(None),
        }
    }

    fn parse_ordered(&mut self, parser: &mut Parser) -> ParseResult<Option<ConceptId>> {
        parser.next();
        let first = self.parse_merge_expression(parser)?.ok_or(ParseError::InvalidSyntax)?;
        let mut components = vec![first];

        loop {
            parser.skip_ws();
            if !parser.consume_str("->") {
                break;
            }

            components.push(self.parse_merge_expression(parser)?.ok_or(ParseError::InvalidSyntax)?);
        }

        if components.len() < 2 {
            return Err(ParseError::InvalidSyntax);
        }
        parser.expect('}')?;
        Ok(Some(self.reference_ordered(components)))
    }

    fn parse_bracket(&mut self, parser: &mut Parser) -> ParseResult<Option<ConceptId>> {
        parser.next();

        if parser.consume('\'') {
            let name = if parser.consume('*') { GLOBAL_PERCEPT_NAME.to_owned() } else { parser.parse_name(true) };
            let percept = self.reference_percept(&name);

            parser.expect('\'')?;
            parser.expect(']')?;

            parser.skip_ws();
            return self.parse_percept_action(parser, percept);
        }

        let name = parser.parse_name(true);
        let concept = self.reference_named(&name);
        parser.expect(']')?;

        Ok(concept)
    }

    fn parse_percept_action(&mut self, parser: &mut Parser, percept: ConceptId) -> ParseResult<Option<ConceptId>> {
        enum Action {
            Assign,
            Add,
            Subtract,
            Merge,
            InverseMerge,
            Experience,
        }

        let action = if parser.consume_str("+=") {
            Action::Add
        } else if parser.consume_str("-=") {
            Action::Subtract
        } else if parser.consume_str("*=") {
            Action::Merge
        } else if parser.consume_str("/=") {
            Action::InverseMerge
        } else if parser.consume_str("~=") {
            Action::Experience
        } else if parser.consume('=') {
            Action::Assign
        } else {
            return Ok(Some(percept));
        };

        if self.is_global_percept(&percept) {
            return Err(ParseError::InvalidSyntax);
        }

        parser.skip_ws();
        let input = self.parse_expression(parser)?;
        Ok(match action {
            Action::Assign => {
                self.set_percept_value(&percept, input.clone());
                input
            }
            Action::Add => self.perform_addition(&percept, input.as_ref()),
            Action::Subtract => self.perform_subtraction(&percept, input.as_ref()),
            Action::Merge => self.perform_merge(&percept, input.as_ref()),
            Action::InverseMerge => self.perform_inverse_merge(&percept, input.as_ref()),
            Action::Experience => self.perform_experience(&percept, input.as_ref()),
        })
    }
}

// Concept interning and engine ownership.
impl Pangine {
    fn reference_named(&mut self, name: &str) -> Option<ConceptId> {
        if name.is_empty() {
            return None;
        }

        if let Some(concept) = self.names.get(name).and_then(Weak::upgrade) {
            return Some(ConceptId(concept));
        }

        let concept = self.alloc(ConceptKind::Named(name.to_owned()), ConceptMap::new());
        self.names.insert(name.to_owned(), Rc::downgrade(&concept.0));
        Some(concept)
    }

    fn reference_inversion(&mut self, concept: Option<ConceptId>) -> Option<ConceptId> {
        let mut map = ConceptMap::new();
        self.add_merge_concept(&mut map, concept?, true, Relevance::DEFAULT);
        self.reference_map(&map)
    }

    fn reference_merge_with_inversion(&mut self, left: Option<ConceptId>, right: Option<ConceptId>, right_inversion: bool) -> Option<ConceptId> {
        let mut map = ConceptMap::new();

        if let Some(left) = left {
            self.add_merge_concept(&mut map, left, false, Relevance::DEFAULT);
        }
        if let Some(right) = right {
            self.add_merge_concept(&mut map, right, right_inversion, Relevance::DEFAULT);
        }

        self.reference_map(&map)
    }

    fn reference_union(&mut self, concepts: &[ConceptId]) -> Option<ConceptId> {
        let mut map = ConceptMap::new();

        for concept in concepts.iter().cloned() {
            self.add_union_concept(&mut map, concept, false, Relevance::DEFAULT);
        }

        self.reference_map(&map)
    }

    fn experience_value_map(&mut self, roots: &ExperienceRoots) -> ConceptMap {
        let mut map = ConceptMap::new();
        for (root, &count) in roots {
            self.add_union_concept(&mut map, root.clone(), false, Relevance::new(1.0, count as f32));
        }
        map
    }

    fn sole_default_concept(map: &ConceptMap) -> Option<&ConceptId> {
        let (concept, relevance) = map.first_key_value()?;
        (map.len() == 1 && *relevance == Relevance::DEFAULT).then_some(concept)
    }

    fn reference_map(&mut self, map: &ConceptMap) -> Option<ConceptId> {
        self.prune_indexes();

        if map.is_empty() {
            return None;
        }

        // 3.x returns a sole default-relevance concept directly before interning:
        // 3.x/pangine/src/libpangine/common/pae_pangine.cpp:314-328
        if let Some(concept) = Self::sole_default_concept(map) {
            return Some(concept.clone());
        }

        Some(self.reference_composite(ConceptKind::Unordered, map.clone()))
    }

    fn reference_ordered(&mut self, mut components: Vec<ConceptId>) -> ConceptId {
        if components.len() == 1 {
            return components.pop().unwrap();
        }

        debug_assert!(components.len() >= 2);
        self.reference_composite(ConceptKind::Ordered { components }, ConceptMap::new())
    }

    fn reference_composite(&mut self, kind: ConceptKind, subconcepts: ConceptMap) -> ConceptId {
        self.prune_indexes();

        for concept in self.composites.iter().filter_map(Weak::upgrade) {
            if concept.kind == kind && concept.subconcepts == subconcepts {
                return ConceptId(concept);
            }
        }

        let concept = self.alloc(kind, subconcepts);
        self.composites.push(Rc::downgrade(&concept.0));
        concept
    }

    fn alloc(&self, kind: ConceptKind, subconcepts: ConceptMap) -> ConceptId {
        let index = self.next_concept_id.get();
        self.next_concept_id.set(index + 1);
        ConceptId::new(self.id, index, kind, subconcepts)
    }

    fn owns(&self, concept: &ConceptId) -> bool {
        concept.0.pangine_id == self.id
    }

    fn is_percept(&self, concept: &ConceptId) -> bool {
        self.owns(concept) && matches!(concept.0.kind, ConceptKind::Percept { .. })
    }

    fn is_global_percept(&self, concept: &ConceptId) -> bool {
        self.is_percept(concept)
            && matches!(
                &concept.0.kind,
                ConceptKind::Percept { name } if name == GLOBAL_PERCEPT_NAME
            )
    }

    fn is_mutable_percept(&self, concept: &ConceptId) -> bool {
        self.is_percept(concept) && !self.is_global_percept(concept)
    }

    fn accepts_percept_input(&self, percept: &ConceptId, input: Option<&ConceptId>) -> bool {
        self.is_mutable_percept(percept) && input.is_none_or(|concept| self.owns(concept))
    }

    fn live_ordinary_concepts(&self) -> impl Iterator<Item = ConceptId> + '_ {
        self.names.values().chain(&self.composites).filter_map(Weak::upgrade).map(ConceptId)
    }

    fn global_value(&self) -> Option<ConceptId> {
        let map = self.live_ordinary_concepts().map(|concept| (concept, Relevance::DEFAULT)).collect::<ConceptMap>();

        self.reference_transient_map(map)
    }

    fn set_percept_roots(&mut self, percept: &ConceptId, roots: ExperienceRoots) -> Option<ConceptId> {
        if !self.is_mutable_percept(percept) || roots.iter().any(|(root, &count)| !self.owns(root) || count == 0) {
            return None;
        }

        let index = percept.index();
        let value_map = self.experience_value_map(&roots);
        let value = match roots.first_key_value() {
            Some((root, &1)) if roots.len() == 1 => Some(root.clone()),
            _ => self.reference_map(&value_map),
        };
        if roots.is_empty() {
            self.percept_roots.remove(&index);
            self.percept_value_maps.remove(&index);
            self.percept_values.remove(&index);
        } else {
            self.percept_roots.insert(index, roots);
            self.percept_value_maps.insert(index, value_map);
            match value.clone() {
                Some(value) => {
                    self.percept_values.insert(index, value);
                }
                None => {
                    self.percept_values.remove(&index);
                }
            }
        }
        value
    }

    fn record_experience(&mut self, percept: &ConceptId, experience: &ConceptId) -> Option<()> {
        if !self.accepts_percept_input(percept, Some(experience)) {
            return None;
        }

        let index = percept.index();
        let current_count = self.percept_roots.get(&index).and_then(|roots| roots.get(experience)).copied().unwrap_or_default();
        let next_count = current_count.checked_add(1)?;
        let incremental_value_map = if current_count == 0 {
            let mut value_map = self.percept_value_maps.remove(&index).unwrap_or_else(|| {
                let roots = self.percept_roots.get(&index).cloned().unwrap_or_default();
                self.experience_value_map(&roots)
            });
            self.add_union_concept(&mut value_map, experience.clone(), false, Relevance::DEFAULT);
            Some(value_map)
        } else {
            None
        };
        self.percept_roots.entry(index).or_default().insert(experience.clone(), next_count);
        let value_map = if let Some(value_map) = incremental_value_map {
            value_map
        } else {
            let roots = self.percept_roots[&index].clone();
            self.experience_value_map(&roots)
        };
        self.percept_value_maps.insert(index, value_map);
        self.percept_values.remove(&index);
        Some(())
    }

    fn materialize_percept_value(&mut self, percept: &ConceptId) -> Option<ConceptId> {
        if !self.is_mutable_percept(percept) {
            return None;
        }

        let index = percept.index();
        if let Some(value) = self.percept_values.get(&index) {
            return Some(value.clone());
        }

        let single_root = self.percept_roots.get(&index).and_then(|roots| match roots.first_key_value() {
            Some((root, &1)) if roots.len() == 1 => Some(root.clone()),
            _ => None,
        });
        let value = if let Some(root) = single_root {
            Some(root)
        } else {
            let value_map = self.percept_value_maps.remove(&index).unwrap_or_else(|| {
                let roots = self.percept_roots.get(&index).cloned().unwrap_or_default();
                self.experience_value_map(&roots)
            });
            let value = self.reference_map(&value_map);
            self.percept_value_maps.insert(index, value_map);
            value
        };

        if let Some(value) = value.clone() {
            self.percept_values.insert(index, value);
        }
        value
    }

    fn reference_transient_map(&self, map: ConceptMap) -> Option<ConceptId> {
        if map.is_empty() {
            return None;
        }

        if let Some(concept) = Self::sole_default_concept(&map) {
            return Some(concept.clone());
        }

        Some(self.alloc(ConceptKind::Unordered, map))
    }

    fn prune_indexes(&mut self) {
        self.names.retain(|_, concept| concept.strong_count() > 0);
        self.composites.retain(|concept| concept.strong_count() > 0);
    }

    fn same_subconcepts_ignoring_relevance(concept: &ConceptId, map: &ConceptMap) -> bool {
        if map.is_empty() {
            return false;
        }

        if Self::sole_default_concept(map) == Some(concept) {
            return true;
        }

        let subconcepts = &concept.0.subconcepts;
        map.len() == subconcepts.len() && map.keys().all(|concept| subconcepts.contains_key(concept))
    }
}

// Percept updates and recursive evaluation.
impl Pangine {
    fn question_percepts(&self, selector: &ConceptId) -> Option<Vec<ConceptId>> {
        if self.is_mutable_percept(selector) {
            return Some(vec![selector.clone()]);
        }
        if !matches!(selector.0.kind, ConceptKind::Unordered) {
            return None;
        }

        let percepts = self
            .canonical_entries(&selector.0.subconcepts)
            .into_iter()
            .map(|(concept, relevance)| (self.is_mutable_percept(&concept) && relevance == Relevance::DEFAULT).then_some(concept))
            .collect::<Option<Vec<_>>>()?;
        (!percepts.is_empty()).then_some(percepts)
    }

    fn percept_value_map(&mut self, percept: &ConceptId) -> Option<ConceptMap> {
        if !self.is_percept(percept) {
            return None;
        }

        let mut map = ConceptMap::new();
        if let Some(current) = self.get_value(percept) {
            self.add_merge_concept(&mut map, current, false, Relevance::DEFAULT);
        }
        Some(map)
    }

    fn perform_union_update(&mut self, percept: &ConceptId, concept: Option<ConceptId>, inversion: bool) -> Option<(Option<ConceptId>, Option<ConceptId>)> {
        let mut map = self.percept_union_value_map(percept)?;

        if let Some(concept) = concept {
            self.add_union_concept(&mut map, concept, inversion, Relevance::DEFAULT);
        }

        let value = self.reference_map(&map);
        let stored = self.preserve_unordered_entry_boundary(&map, value.clone());
        Some((value, stored))
    }

    fn perform_merge_update(&mut self, percept: &ConceptId, concept: Option<ConceptId>, inversion: bool) -> Option<ConceptId> {
        let mut map = self.percept_value_map(percept)?;

        if let Some(concept) = concept {
            self.add_merge_concept(&mut map, concept, inversion, Relevance::DEFAULT);
        }

        self.reference_map(&map)
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
        self.add_union_concept(&mut map, current, false, Relevance::DEFAULT);
        Some(map)
    }

    fn preserve_unordered_entry_boundary(&mut self, map: &ConceptMap, value: Option<ConceptId>) -> Option<ConceptId> {
        // Normalization ordinarily collapses a sole default-relevance member.
        // Preserve this boundary when that member is itself unordered so ^
        // chooses the complete member instead of choosing one of its parts.
        if let Some(concept) = Self::sole_default_concept(map) {
            if matches!(concept.0.kind, ConceptKind::Unordered) {
                let stored = self.alloc(ConceptKind::Unordered, map.clone());
                self.composites.push(Rc::downgrade(&stored.0));
                return Some(stored);
            }
        }

        value
    }

    fn answer_question(&mut self, percepts: &[ConceptId], question: Option<ConceptId>) -> Option<ConceptId> {
        let question = question?;
        let snapshot = self.question_snapshot(percepts, &question);

        let projection_results = self.get_projection_results(&question, &snapshot);

        for (percept, binding_result) in projection_results {
            self.set_percept_value(&percept, binding_result);
        }

        Some(question)
    }

    fn question_snapshot(&mut self, percepts: &[ConceptId], question: &ConceptId) -> QuestionSnapshot {
        let sources = percepts
            .iter()
            .flat_map(|percept| {
                self.percept_roots
                    .get(&percept.index())
                    .into_iter()
                    .flatten()
                    .map(|(root, &occurrences)| QuestionSource { percept: percept.clone(), root: root.clone(), occurrences })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();

        let mut patterns = BTreeSet::new();
        let mut contains_percept_cache = BTreeMap::new();
        self.collect_question_patterns(question, true, &mut patterns, &mut contains_percept_cache);
        // A root output Percept may bind any recursive view. Every other
        // pattern can only match its own structural shape.
        let experience_shapes =
            (!patterns.iter().any(|pattern| self.is_percept(pattern))).then(|| patterns.iter().map(|pattern| pattern.0.shape()).collect::<BTreeSet<_>>());
        let needs_context = patterns
            .iter()
            .any(|pattern| pattern.0.ordered_components().and_then(|components| components.first()).is_some_and(|origin| self.origin_has_route_seed(origin)));
        let mut ordered_widths = BTreeSet::new();
        self.collect_ordered_question_widths(question, &mut BTreeSet::new(), &mut BTreeMap::new(), &mut ordered_widths);
        let mut snapshot = QuestionSnapshot::default();
        for source in sources {
            self.add_question_experience_rec(
                &source,
                &source.root,
                &ordered_widths,
                experience_shapes.as_ref(),
                &mut BTreeSet::new(),
                &mut snapshot.experiences,
            );
            if needs_context {
                Self::add_question_graph_rec(&source, &source.root, &mut BTreeSet::new(), &mut snapshot.source_concepts, &mut snapshot.graph);
            }
        }
        snapshot
    }

    fn collect_ordered_question_widths(
        &self,
        concept: &ConceptId,
        visited: &mut BTreeSet<ConceptId>,
        contains_percept_cache: &mut BTreeMap<usize, bool>,
        widths: &mut BTreeSet<usize>,
    ) {
        if !visited.insert(concept.clone()) || !self.contains_percept(concept, contains_percept_cache) {
            return;
        }

        if let ConceptKind::Ordered { components } = &concept.0.kind {
            widths.insert(components.len());
        }
        for (child, _) in concept.0.children() {
            self.collect_ordered_question_widths(child, visited, contains_percept_cache, widths);
        }
    }

    fn add_question_experience_rec(
        &mut self,
        source: &QuestionSource,
        concept: &ConceptId,
        ordered_widths: &BTreeSet<usize>,
        experience_shapes: Option<&BTreeSet<ConceptShape>>,
        visited: &mut BTreeSet<ConceptId>,
        experiences: &mut BTreeSet<QuestionExperience>,
    ) {
        if !visited.insert(concept.clone()) {
            return;
        }
        if experience_shapes.is_none_or(|shapes| shapes.contains(&concept.0.shape())) {
            let blocked_memberships = match &concept.0.kind {
                ConceptKind::Ordered { components } => {
                    (0..components.len()).map(|position| QuestionMembership { parent: concept.clone(), port: QuestionPort::Ordered(position) }).collect()
                }
                ConceptKind::Named(_) | ConceptKind::Percept { .. } | ConceptKind::Unordered => BTreeSet::new(),
            };
            experiences.insert(QuestionExperience { source: source.clone(), matched: concept.clone(), blocked_memberships });
        }

        match &concept.0.kind {
            ConceptKind::Ordered { components } => {
                let components = components.clone();
                for &width in ordered_widths.range(2..components.len()) {
                    if experience_shapes.is_some_and(|shapes| !shapes.contains(&ConceptShape::Ordered(width))) {
                        continue;
                    }
                    for (start, window) in components.windows(width).enumerate() {
                        let matched = self.reference_ordered(window.to_vec());
                        let blocked_memberships = (start..start + width)
                            .map(|position| QuestionMembership { parent: concept.clone(), port: QuestionPort::Ordered(position) })
                            .collect();
                        experiences.insert(QuestionExperience { source: source.clone(), matched, blocked_memberships });
                    }
                }
                for child in components {
                    self.add_question_experience_rec(source, &child, ordered_widths, experience_shapes, visited, experiences);
                }
            }
            ConceptKind::Unordered => {
                let children = concept.0.subconcepts.clone();
                for (child, relevance) in children {
                    // reference_map returns a sole default member directly.
                    // Avoid global interner cleanup for that common case.
                    let weighted = if relevance == Relevance::DEFAULT { Some(child) } else { self.reference_map(&ConceptMap::from([(child, relevance)])) };
                    if let Some(weighted) = weighted {
                        self.add_question_experience_rec(source, &weighted, ordered_widths, experience_shapes, visited, experiences);
                    }
                }
            }
            ConceptKind::Named(_) | ConceptKind::Percept { .. } => {}
        }
    }

    fn add_question_graph_rec(
        source: &QuestionSource,
        concept: &ConceptId,
        visited: &mut BTreeSet<ConceptId>,
        source_concepts: &mut BTreeSet<QuestionSeed>,
        graph: &mut QuestionGraph,
    ) {
        if !visited.insert(concept.clone()) {
            return;
        }
        source_concepts.insert(QuestionSeed { source: source.clone(), concept: concept.clone() });

        match &concept.0.kind {
            ConceptKind::Ordered { components } => {
                for (position, child) in components.iter().enumerate() {
                    graph.add_membership(source, concept, child, QuestionPort::Ordered(position));
                    Self::add_question_graph_rec(source, child, visited, source_concepts, graph);
                }
            }
            ConceptKind::Unordered => {
                for child in concept.0.subconcepts.keys() {
                    graph.add_membership(source, concept, child, QuestionPort::Unordered(child.clone()));
                    Self::add_question_graph_rec(source, child, visited, source_concepts, graph);
                }
            }
            ConceptKind::Named(_) | ConceptKind::Percept { .. } => {}
        }
    }

    fn evaluate_concept(&mut self, concept: &ConceptId) -> Option<ConceptId> {
        self.evaluate_concept_inner(concept, &mut BTreeSet::new())
    }

    fn evaluate_concept_inner(&mut self, concept: &ConceptId, visited_percepts: &mut BTreeSet<ConceptId>) -> Option<ConceptId> {
        match &concept.0.kind {
            ConceptKind::Named(_) => Some(concept.clone()),
            ConceptKind::Percept { .. } => {
                if !visited_percepts.insert(concept.clone()) {
                    return Some(concept.clone());
                }

                let evaluated = self.get_value(concept).and_then(|value| {
                    if self.is_global_percept(concept) {
                        self.evaluate_transient_concept(&value, visited_percepts)
                    } else {
                        self.evaluate_concept_inner(&value, visited_percepts)
                    }
                });
                visited_percepts.remove(concept);
                evaluated
            }
            ConceptKind::Unordered => {
                let evaluated = self.evaluate_subconcepts(concept, visited_percepts);
                self.reference_map(&evaluated)
            }
            ConceptKind::Ordered { components } => {
                let components = components.clone();
                let mut evaluated = Vec::with_capacity(components.len());
                for component in components {
                    evaluated.push(self.evaluate_concept_inner(&component, visited_percepts)?);
                }
                Some(self.reference_ordered(evaluated))
            }
        }
    }

    fn evaluate_transient_concept(&mut self, concept: &ConceptId, visited_percepts: &mut BTreeSet<ConceptId>) -> Option<ConceptId> {
        if matches!(concept.0.kind, ConceptKind::Unordered) {
            let evaluated = self.evaluate_subconcepts(concept, visited_percepts);
            self.reference_transient_map(evaluated)
        } else {
            self.evaluate_concept_inner(concept, visited_percepts)
        }
    }

    fn evaluate_subconcepts(&mut self, concept: &ConceptId, visited_percepts: &mut BTreeSet<ConceptId>) -> ConceptMap {
        let mut evaluated = ConceptMap::new();
        for (child, relevance) in concept.0.subconcepts.clone() {
            if let Some(child) = self.evaluate_concept_inner(&child, visited_percepts) {
                self.add_relevance(&mut evaluated, child, false, relevance);
            }
        }
        evaluated
    }
}

// Experience/question projection.
impl Pangine {
    fn get_projection_results(&mut self, question: &ConceptId, snapshot: &QuestionSnapshot) -> BTreeMap<ConceptId, Option<ConceptId>> {
        let mut questions = BTreeSet::new();
        let mut contains_percept_cache = BTreeMap::new();
        self.collect_question_patterns(question, true, &mut questions, &mut contains_percept_cache);

        let mut output_percepts = BTreeSet::new();
        self.collect_output_percepts(question, &mut output_percepts);
        let mut witnesses = output_percepts.iter().cloned().map(|percept| (percept, BTreeMap::new())).collect::<QuestionCandidateWitnesses>();
        let mut experience_index = BTreeMap::<ConceptShape, Vec<&QuestionExperience>>::new();
        for experience in &snapshot.experiences {
            experience_index.entry(experience.matched.0.shape()).or_default().push(experience);
        }
        let mut cache = ProjectionCache::new();

        for question in &questions {
            let matching_experiences = if self.is_percept(question) {
                snapshot.experiences.iter().collect::<Vec<_>>()
            } else {
                experience_index.get(&question.0.shape()).into_iter().flatten().copied().collect::<Vec<_>>()
            };

            for experience in matching_experiences {
                if let Some(assignment) = self.unordered_remainder_assignment(&experience.matched, question) {
                    Self::add_question_assignment(&experience.source, &assignment, &output_percepts, &mut witnesses);
                }

                let assignments = self.projection_assignments(&experience.matched, question, &mut cache);
                Self::add_projection_witnesses(&experience.source, &assignments, &output_percepts, &mut witnesses);
            }

            self.add_contextual_projection_witnesses(question, &output_percepts, &mut cache, snapshot, &mut witnesses);
        }

        self.materialize_question_witnesses(output_percepts, witnesses)
    }

    fn add_projection_witnesses(
        source: &QuestionSource,
        assignments: &ProjectionAssignments,
        output_percepts: &BTreeSet<ConceptId>,
        witnesses: &mut QuestionCandidateWitnesses,
    ) {
        for assignment in assignments {
            Self::add_question_assignment(source, assignment, output_percepts, witnesses);
        }
    }

    fn add_question_assignment(
        source: &QuestionSource,
        assignment: &ProjectionAssignment,
        output_percepts: &BTreeSet<ConceptId>,
        witnesses: &mut QuestionCandidateWitnesses,
    ) {
        if output_percepts.iter().any(|percept| !assignment.contains_key(percept)) {
            return;
        }

        for output in output_percepts {
            let candidate = assignment[output].clone();
            witnesses.entry(output.clone()).or_default().entry(candidate).or_default().insert(source.clone());
        }
    }

    fn materialize_question_witnesses(
        &mut self,
        output_percepts: BTreeSet<ConceptId>,
        mut witnesses: QuestionCandidateWitnesses,
    ) -> BTreeMap<ConceptId, Option<ConceptId>> {
        let mut results = BTreeMap::new();
        for output in output_percepts {
            let mut candidates = ConceptMap::new();
            for (candidate, candidate_witnesses) in witnesses.remove(&output).unwrap_or_default() {
                // Question strength is a support count for deterministic
                // choice, not a calibrated probability.
                let support = candidate_witnesses.iter().map(|source| u128::from(source.occurrences)).sum::<u128>();
                self.add_relevance(&mut candidates, candidate, false, Relevance::new(1.0, support as f32));
            }
            let value = self.reference_map(&candidates);
            results.insert(output, self.preserve_unordered_entry_boundary(&candidates, value));
        }
        results
    }

    fn add_contextual_projection_witnesses(
        &mut self,
        question: &ConceptId,
        output_percepts: &BTreeSet<ConceptId>,
        cache: &mut ProjectionCache,
        snapshot: &QuestionSnapshot,
        witnesses: &mut QuestionCandidateWitnesses,
    ) {
        let Some(question_components) = question.0.ordered_components() else {
            return;
        };
        let Some(origin_pattern) = question_components.first() else {
            return;
        };
        if !self.origin_has_route_seed(origin_pattern) {
            return;
        }

        let mut origin_cache = ProjectionCache::new();
        let origins = snapshot
            .source_concepts
            .iter()
            .map(|seed| &seed.concept)
            .filter(|concept| !self.projection_assignments(concept, origin_pattern, &mut origin_cache).is_empty())
            .cloned()
            .collect::<BTreeSet<_>>();
        if origins.is_empty() {
            return;
        }

        for answer in &snapshot.experiences {
            let Some(answer_components) = answer.matched.0.ordered_components() else {
                continue;
            };
            if answer_components.len() != question_components.len() {
                continue;
            }
            let Some(answer_origin) = answer_components.first() else {
                continue;
            };

            for origin in &origins {
                if origin == answer_origin || !snapshot.graph.connects(origin, answer_origin, answer) {
                    continue;
                }

                let mut routed_components = answer_components.to_vec();
                routed_components[0] = origin.clone();
                let routed = self.reference_ordered(routed_components);
                let assignments = self.projection_assignments(&routed, question, cache);
                if assignments.is_empty() {
                    continue;
                }
                Self::add_projection_witnesses(&answer.source, &assignments, output_percepts, witnesses);
            }
        }
    }

    fn origin_has_route_seed(&self, concept: &ConceptId) -> bool {
        match &concept.0.kind {
            ConceptKind::Named(_) => true,
            ConceptKind::Percept { .. } => false,
            ConceptKind::Ordered { components } => components.first().is_some_and(|component| self.origin_has_route_seed(component)),
            ConceptKind::Unordered => concept.0.subconcepts.keys().any(|member| self.contains_fixed_question_concept(member)),
        }
    }

    fn contains_fixed_question_concept(&self, concept: &ConceptId) -> bool {
        !self.is_percept(concept)
            && (matches!(concept.0.kind, ConceptKind::Named(_)) || concept.0.children().any(|(child, _)| self.contains_fixed_question_concept(child)))
    }

    fn collect_output_percepts(&self, concept: &ConceptId, percepts: &mut BTreeSet<ConceptId>) {
        if self.is_percept(concept) {
            percepts.insert(concept.clone());
            return;
        }

        for (child, _) in concept.0.children() {
            self.collect_output_percepts(child, percepts);
        }
    }

    fn collect_question_patterns(
        &self,
        question: &ConceptId,
        is_root: bool,
        patterns: &mut BTreeSet<ConceptId>,
        contains_percept_cache: &mut BTreeMap<usize, bool>,
    ) {
        if !self.contains_percept(question, contains_percept_cache) {
            return;
        }

        if is_root || !self.is_percept(question) {
            patterns.insert(question.clone());
        }

        if matches!(question.0.kind, ConceptKind::Ordered { .. }) {
            return;
        }

        for (child, _) in question.0.children() {
            self.collect_question_patterns(child, false, patterns, contains_percept_cache);
        }
    }

    fn contains_percept(&self, concept: &ConceptId, cache: &mut BTreeMap<usize, bool>) -> bool {
        if let Some(contains) = cache.get(&concept.index()) {
            return *contains;
        }

        let contains = self.is_percept(concept) || concept.0.children().any(|(child, _)| self.contains_percept(child, cache));
        cache.insert(concept.index(), contains);
        contains
    }

    fn projection_assignments(&self, experience: &ConceptId, question: &ConceptId, cache: &mut ProjectionCache) -> ProjectionAssignments {
        let key = (experience.index(), question.index());
        if let Some(assignments) = cache.get(&key) {
            return assignments.clone();
        }

        let assignments = if self.is_percept(question) {
            ProjectionAssignments::from([ProjectionAssignment::from([(question.clone(), experience.clone())])])
        } else if let (ConceptKind::Named(experience_name), ConceptKind::Named(question_name)) = (&experience.0.kind, &question.0.kind) {
            if experience_name == question_name {
                Self::exact_projection_assignments()
            } else {
                ProjectionAssignments::new()
            }
        } else if let (Some(experience_components), Some(question_components)) = (experience.0.ordered_components(), question.0.ordered_components()) {
            if experience_components.len() != question_components.len() {
                ProjectionAssignments::new()
            } else {
                let mut ordered = Self::exact_projection_assignments();
                for (experience_component, question_component) in experience_components.iter().zip(question_components) {
                    let component = self.projection_assignments(experience_component, question_component, cache);
                    ordered = Self::multiply_projection_assignments(&ordered, &component);
                    if ordered.is_empty() {
                        break;
                    }
                }
                ordered
            }
        } else if experience.0.shape() == question.0.shape() && matches!(experience.0.shape(), ConceptShape::Unordered) {
            self.unordered_projection_assignments(experience, question, cache)
        } else {
            ProjectionAssignments::new()
        };

        cache.insert(key, assignments.clone());
        assignments
    }

    fn exact_projection_assignments() -> ProjectionAssignments {
        ProjectionAssignments::from([ProjectionAssignment::new()])
    }

    fn multiply_projection_assignments(left: &ProjectionAssignments, right: &ProjectionAssignments) -> ProjectionAssignments {
        let mut products = ProjectionAssignments::new();
        for left in left {
            for right in right {
                if let Some(product) = Self::merge_projection_assignments(left, right) {
                    products.insert(product);
                }
            }
        }
        products
    }

    fn merge_projection_assignments(left: &ProjectionAssignment, right: &ProjectionAssignment) -> Option<ProjectionAssignment> {
        let mut merged = left.clone();
        for (percept, candidate) in right {
            if let Some(current) = merged.get(percept) {
                if current != candidate {
                    return None;
                }
            } else {
                merged.insert(percept.clone(), candidate.clone());
            }
        }
        Some(merged)
    }

    // Unequal unions currently bind one direct output to the complete remainder
    // left by an exact, default-relevance subset. Relevance-aware and partial
    // subset matching remain open questions.
    fn unordered_remainder_assignment(&mut self, experience: &ConceptId, question: &ConceptId) -> Option<ProjectionAssignment> {
        if experience.0.shape() != ConceptShape::Unordered || question.0.shape() != ConceptShape::Unordered {
            return None;
        }

        let experiences = experience.0.subconcepts.iter().map(|(concept, &relevance)| (concept.clone(), relevance)).collect::<Vec<_>>();
        let questions = question.0.subconcepts.iter().map(|(concept, &relevance)| (concept.clone(), relevance)).collect::<Vec<_>>();
        if experiences.len() <= questions.len()
            || Self::has_non_default_relevance(experience, &mut BTreeSet::new())
            || Self::has_non_default_relevance(question, &mut BTreeSet::new())
        {
            return None;
        }

        let outputs = questions.iter().filter(|(concept, _)| self.is_percept(concept)).collect::<Vec<_>>();
        if outputs.len() != 1 {
            return None;
        }
        let (output, _) = outputs[0];
        let fixed_questions = questions.iter().filter(|(concept, _)| concept != output).collect::<Vec<_>>();
        let mut contains_percept_cache = BTreeMap::new();
        if fixed_questions.is_empty()
            || fixed_questions.iter().any(|(concept, _)| self.contains_percept(concept, &mut contains_percept_cache))
            || fixed_questions.iter().any(|(question, _)| !experience.0.subconcepts.contains_key(question))
        {
            return None;
        }

        let fixed_concepts = fixed_questions.iter().map(|(concept, _)| concept).collect::<BTreeSet<_>>();
        let remainder = experiences.into_iter().filter(|(concept, _)| !fixed_concepts.contains(concept)).collect::<ConceptMap>();
        let remainder = self.reference_map(&remainder)?;
        Some(ProjectionAssignment::from([(output.clone(), remainder)]))
    }

    fn has_non_default_relevance(concept: &ConceptId, visited: &mut BTreeSet<ConceptId>) -> bool {
        if !visited.insert(concept.clone()) {
            return false;
        }

        concept.0.subconcepts.values().any(|&relevance| relevance != Relevance::DEFAULT)
            || concept.0.children().any(|(child, _)| Self::has_non_default_relevance(child, visited))
    }

    fn unordered_projection_assignments(&self, experience: &ConceptId, question: &ConceptId, cache: &mut ProjectionCache) -> ProjectionAssignments {
        let experiences = experience.0.subconcepts.iter().collect::<Vec<_>>();
        let questions = question.0.subconcepts.iter().collect::<Vec<_>>();
        if experiences.len() != questions.len() {
            return ProjectionAssignments::new();
        }

        let Some(state_count) = 1usize.checked_shl(experiences.len() as u32) else {
            return ProjectionAssignments::new();
        };
        let edges = questions
            .iter()
            .map(|(question, _)| experiences.iter().map(|(experience, _)| self.projection_assignments(experience, question, cache)).collect::<Vec<_>>())
            .collect::<Vec<_>>();

        let mut forward = vec![ProjectionAssignments::new(); state_count];
        forward[0] = Self::exact_projection_assignments();
        for mask in 0..state_count {
            let question_index = mask.count_ones() as usize;
            if question_index == questions.len() {
                continue;
            }
            let current = forward[mask].clone();
            if current.is_empty() {
                continue;
            }

            for (experience_index, edge) in edges[question_index].iter().enumerate() {
                let bit = 1usize << experience_index;
                if mask & bit != 0 {
                    continue;
                }

                let products = Self::multiply_projection_assignments(&current, edge);
                forward[mask | bit].extend(products);
            }
        }

        forward.pop().unwrap_or_default()
    }
}

// Relevance accumulation and structural access.
impl Pangine {
    fn add_merge_concept(&mut self, map: &mut ConceptMap, concept: ConceptId, inversion: bool, relevance: Relevance) {
        let subconcepts = concept.0.subconcepts.clone();
        if matches!(concept.0.kind, ConceptKind::Unordered) {
            for (child, child_relevance) in subconcepts {
                self.add_union_concept(map, child, inversion, multiply_relevance(relevance, child_relevance));
            }
        } else {
            self.add_union_concept(map, concept, inversion, relevance);
        }
    }

    fn add_union_concept(&mut self, map: &mut ConceptMap, concept: ConceptId, inversion: bool, relevance: Relevance) {
        let subconcepts = concept.0.subconcepts.clone();
        if matches!(concept.0.kind, ConceptKind::Unordered) && subconcepts.len() == 1 {
            let (child, child_relevance) = subconcepts.into_iter().next().unwrap();
            self.add_union_concept(map, child, inversion, multiply_relevance(relevance, child_relevance));
        } else {
            self.add_relevance(map, concept, inversion, relevance);
        }
    }

    fn add_relevance_map(&mut self, target: &mut ConceptMap, source: ConceptMap, relevance: Relevance) {
        for (concept, source_relevance) in source {
            let mut current = relevance;
            current.probability = source_relevance.probability;
            current.strength *= source_relevance.strength;
            self.add_relevance(target, concept, false, current);
        }
    }

    fn add_relevance(&mut self, map: &mut ConceptMap, concept: ConceptId, inversion: bool, mut relevance: Relevance) {
        if inversion {
            relevance.strength = -relevance.strength;
        }

        let concept_subconcepts = concept.0.subconcepts.clone();
        let found = if map.contains_key(&concept) {
            Some((concept.clone(), false))
        } else if matches!(concept.0.kind, ConceptKind::Unordered) {
            map.keys()
                .find(|candidate| {
                    matches!(candidate.0.kind, ConceptKind::Unordered) && Self::same_subconcepts_ignoring_relevance(candidate, &concept_subconcepts)
                })
                .cloned()
                .map(|candidate| (candidate, true))
        } else {
            None
        };

        match found {
            None => {
                if !relevance.is_empty() {
                    map.insert(concept, relevance);
                }
            }
            Some((existing, true)) => {
                let existing_relevance = map[&existing];
                let mut new_map = existing.0.subconcepts.clone();

                for value in new_map.values_mut() {
                    value.strength *= existing_relevance.strength;
                }

                map.remove(&existing);
                self.add_relevance_map(&mut new_map, concept_subconcepts, Relevance::DEFAULT);

                if let Some(result) = self.reference_map(&new_map) {
                    self.add_relevance(map, result, inversion, relevance);
                }
            }
            Some((existing, false)) => {
                if let Some(current) = map.get_mut(&existing) {
                    current.add(relevance);
                    if current.is_empty() {
                        map.remove(&existing);
                    }
                }
            }
        }
    }

    fn make_decision(&self, concept: &ConceptId) -> Option<ConceptId> {
        let concept = self.get_value(concept)?;
        if !matches!(concept.0.kind, ConceptKind::Unordered) {
            return Some(concept);
        }

        let mut selected: Option<(f32, String, ConceptId)> = None;
        for (candidate, relevance) in &concept.0.subconcepts {
            let weight = relevance.weight();
            if !weight.is_finite() || weight <= 0.0 {
                continue;
            }

            let canonical = self.format_concept(candidate, false);
            let replace = match &selected {
                None => true,
                Some((greatest, earliest, _)) => weight > *greatest || (weight == *greatest && canonical < *earliest),
            };
            if replace {
                selected = Some((weight, canonical, candidate.clone()));
            }
        }

        selected.map(|(_, _, candidate)| candidate)
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

// Canonical and diagnostic formatting.
impl Pangine {
    fn format_inner(&self, concept: &ConceptId, evaluate: bool, active: &mut BTreeSet<ConceptId>) -> String {
        if !active.insert(concept.clone()) {
            return match &concept.0.kind {
                ConceptKind::Named(name) => format!("[{name}]"),
                ConceptKind::Percept { name } => format!("['{name}']"),
                _ => format!("[#{}]", concept.index()),
            };
        }

        let formatted = match &concept.0.kind {
            ConceptKind::Named(name) => format!("[{name}]"),
            ConceptKind::Percept { name } => {
                if evaluate {
                    self.get_value(concept).map_or_else(|| "[]".to_owned(), |value| self.format_inner(&value, evaluate, active))
                } else {
                    format!("['{name}']")
                }
            }
            ConceptKind::Ordered { components } => {
                let mut ordered = String::from("{");
                for (index, component) in components.iter().enumerate() {
                    if index > 0 {
                        ordered.push_str("->");
                    }
                    ordered.push_str(&self.format_inner(component, evaluate, active));
                }
                ordered.push('}');
                ordered
            }
            ConceptKind::Unordered => self.format_relevance(&concept.0.subconcepts, evaluate, active),
        };

        active.remove(concept);
        formatted
    }

    fn canonical_entries(&self, map: &ConceptMap) -> Vec<(ConceptId, Relevance)> {
        let mut entries: Vec<_> = map.iter().map(|(concept, &relevance)| (concept.clone(), relevance)).collect();

        entries.sort_by(|(left_concept, left_relevance), (right_concept, right_relevance)| {
            compare_canonical_relevance_desc(*left_relevance, *right_relevance).then_with(|| self.compare_concepts(left_concept, right_concept))
        });
        entries
    }

    // 3.x orders concepts by percept/name, union shape, relevance, and semantic
    // components rather than allocation order:
    // 3.x/pangine/src/libpangine/common/pae_concept.cpp:15
    fn compare_concepts(&self, left: &ConceptId, right: &ConceptId) -> Ordering {
        if left == right {
            return Ordering::Equal;
        }

        let left_kind = &left.0.kind;
        let right_kind = &right.0.kind;
        let left_is_percept = matches!(left_kind, ConceptKind::Percept { .. });
        let right_is_percept = matches!(right_kind, ConceptKind::Percept { .. });

        if left_is_percept != right_is_percept {
            return right_is_percept.cmp(&left_is_percept);
        }

        let left_name = match left_kind {
            ConceptKind::Named(name) | ConceptKind::Percept { name } => Some(name),
            _ => None,
        };
        let right_name = match right_kind {
            ConceptKind::Named(name) | ConceptKind::Percept { name } => Some(name),
            _ => None,
        };

        if let (Some(left_name), Some(right_name)) = (left_name, right_name) {
            let order = left_name.cmp(right_name);
            if order != Ordering::Equal {
                return order;
            }
        }

        let left_subconcepts = &left.0.subconcepts;
        let right_subconcepts = &right.0.subconcepts;
        let order = left_subconcepts.len().cmp(&right_subconcepts.len());
        if order != Ordering::Equal {
            return order;
        }

        for ((left_concept, left_relevance), (right_concept, right_relevance)) in
            self.canonical_entries(left_subconcepts).into_iter().zip(self.canonical_entries(right_subconcepts))
        {
            let order = compare_canonical_relevance_desc(left_relevance, right_relevance);
            if order != Ordering::Equal {
                return order;
            }

            let order = self.compare_concepts(&left_concept, &right_concept);
            if order != Ordering::Equal {
                return order;
            }
        }

        match (left.0.ordered_components(), right.0.ordered_components()) {
            (Some(left_components), Some(right_components)) => {
                let order = left_components.len().cmp(&right_components.len());
                if order != Ordering::Equal {
                    return order;
                }
                for (left_component, right_component) in left_components.iter().zip(right_components) {
                    let order = self.compare_concepts(left_component, right_component);
                    if order != Ordering::Equal {
                        return order;
                    }
                }
            }
            (Some(_), None) => return Ordering::Greater,
            (None, Some(_)) => return Ordering::Less,
            (None, None) => {}
        }

        left.cmp(right)
    }

    fn format_relevance(&self, map: &ConceptMap, evaluate: bool, active: &mut BTreeSet<ConceptId>) -> String {
        let mut out = String::new();

        for (concept, relevance) in self.canonical_entries(map) {
            if relevance.probability != 1.0 {
                out.push_str(&format_relevance_probability(relevance));
            }

            out.push_str(&format_relevance_strength(relevance));
            let wrap_concept = matches!(concept.0.kind, ConceptKind::Unordered);
            if wrap_concept {
                out.push('(');
            }
            out.push_str(&self.format_inner(&concept, evaluate, active));
            if wrap_concept {
                out.push(')');
            }
        }

        out
    }

    fn format_debug_console_line(&self, relevance: Relevance, concept: &ConceptId) -> String {
        let mut out = String::from("  ");
        let add_separator = relevance.probability != 1.0 || (relevance.strength != 1.0 && relevance.strength != -1.0);

        if relevance.strength == -1.0 {
            out.push('!');
        }

        if relevance.probability != 1.0 {
            out.push_str(&format_relevance_probability(relevance));
        }

        if relevance.strength != 1.0 && relevance.strength != -1.0 {
            out.push_str(&format_relevance_strength(relevance));
        }

        if add_separator {
            out.push(' ');
        }

        out.push_str(&self.format_concept(concept, false));
        out
    }
}

struct Parser {
    chars: Vec<char>,
    pos: usize,
}

impl Parser {
    fn new(script: &str) -> Self {
        Self { chars: script.chars().collect(), pos: 0 }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn peek_next(&self) -> Option<char> {
        self.chars.get(self.pos + 1).copied()
    }

    fn next(&mut self) -> Option<char> {
        let current = self.peek()?;
        self.pos += 1;
        Some(current)
    }

    fn consume(&mut self, expected: char) -> bool {
        if self.peek() == Some(expected) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn consume_str(&mut self, expected: &str) -> bool {
        let len = expected.chars().count();
        if expected.chars().enumerate().all(|(i, ch)| self.chars.get(self.pos + i) == Some(&ch)) {
            self.pos += len;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, expected: char) -> ParseResult<()> {
        self.consume(expected).then_some(()).ok_or(ParseError::InvalidSyntax)
    }

    fn skip_ws(&mut self) {
        loop {
            while self.peek().is_some_and(char::is_whitespace) {
                self.pos += 1;
            }

            match (self.peek(), self.peek_next()) {
                (Some('/'), Some('/')) => self.skip_line_comment(),
                (Some('/'), Some('*')) => {
                    if !self.skip_block_comment() {
                        return;
                    }
                }
                _ => return,
            }
        }
    }

    fn skip_line_comment(&mut self) {
        while self.peek().is_some_and(|c| c != '\n' && c != '\r') {
            self.pos += 1;
        }
    }

    fn skip_block_comment(&mut self) -> bool {
        let start = self.pos;
        self.pos += 2;

        while self.peek().is_some() {
            if self.peek() == Some('*') && self.peek_next() == Some('/') {
                self.pos += 2;
                return true;
            }
            self.pos += 1;
        }

        self.pos = start;
        false
    }

    fn parse_name(&mut self, allow_space: bool) -> String {
        let start = self.pos;
        while self.peek().is_some_and(|c| is_name_char(c, allow_space)) {
            self.pos += 1;
        }
        self.chars[start..self.pos].iter().collect()
    }

    fn parse_probability(&mut self) -> f32 {
        let start = self.pos;
        if let Some(value) = self.parse_number() {
            if self.consume('%') {
                return value / 100.0;
            }
        }

        self.pos = start;
        1.0
    }

    fn parse_float(&mut self) -> f32 {
        self.parse_number().unwrap_or(0.0)
    }

    fn starts_union_operand(&mut self) -> bool {
        if self.peek().is_some_and(|c| matches!(c, '(' | '[' | '{' | '$' | '^' | '!' | 'x')) {
            return true;
        }

        let start = self.pos;
        self.parse_probability();
        let starts = self.pos != start;
        self.pos = start;
        starts
    }

    fn parse_number(&mut self) -> Option<f32> {
        let start = self.pos;

        if self.peek() == Some('-') {
            self.pos += 1;
        }

        let mut has_digit = false;
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            has_digit = true;
            self.pos += 1;
        }

        if self.peek() == Some('.') {
            self.pos += 1;
            while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                has_digit = true;
                self.pos += 1;
            }
        }

        if !has_digit {
            self.pos = start;
            return None;
        }

        let Ok(value) = self.chars[start..self.pos].iter().collect::<String>().parse() else {
            self.pos = start;
            return None;
        };

        Some(value)
    }
}

fn is_name_char(c: char, allow_space: bool) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-' || (allow_space && c == ' ')
}

fn debug_console_help(command: &str) -> Option<&'static str> {
    matches!(command, "h" | "help").then_some(DEBUG_CONSOLE_HELP)
}

fn debug_console_quit(command: &str) -> bool {
    matches!(command, "q" | "quit")
}

fn statement_has_tokens(statement: &str) -> bool {
    let mut parser = Parser::new(statement);
    parser.skip_ws();
    parser.peek().is_some()
}

struct ScriptStatements<'a> {
    items: Vec<&'a str>,
    has_semicolons: bool,
}

fn split_script_statements(script: &str) -> ScriptStatements<'_> {
    let mut statements = Vec::new();
    let mut stack = Vec::new();
    let mut start = 0;
    let mut has_semicolons = false;
    let mut in_block_comment = false;
    let mut in_line_comment = false;
    let mut split_before_line_comment = false;
    let mut chars = script.char_indices().peekable();

    while let Some((index, ch)) = chars.next() {
        if in_block_comment {
            if ch == '*' && chars.peek().is_some_and(|(_, next)| *next == '/') {
                chars.next();
                in_block_comment = false;
            }
            continue;
        }

        if in_line_comment {
            if ch == '\n' || ch == '\r' {
                in_line_comment = false;
                if stack.is_empty() {
                    if !split_before_line_comment {
                        statements.push(&script[start..index]);
                    }
                    start = index + ch.len_utf8();
                }
                split_before_line_comment = false;
            }
            continue;
        }

        match ch {
            '#' if stack.is_empty() => {
                statements.push(&script[start..index]);
                in_line_comment = true;
                split_before_line_comment = true;
            }
            '/' if chars.peek().is_some_and(|(_, next)| *next == '/') => {
                chars.next();
                in_line_comment = true;
            }
            '/' if chars.peek().is_some_and(|(_, next)| *next == '*') => {
                chars.next();
                in_block_comment = true;
            }
            ';' if stack.is_empty() => {
                has_semicolons = true;
                statements.push(&script[start..index]);
                start = index + ch.len_utf8();
            }
            '\n' | '\r' if stack.is_empty() => {
                statements.push(&script[start..index]);
                start = index + ch.len_utf8();
            }
            '(' => stack.push(')'),
            '[' => stack.push(']'),
            '{' => stack.push('}'),
            ')' | ']' | '}' if stack.last() == Some(&ch) => {
                stack.pop();
            }
            _ => {}
        }
    }

    if in_line_comment && split_before_line_comment {
        start = script.len();
    }
    statements.push(&script[start..]);
    ScriptStatements { items: statements, has_semicolons }
}

fn multiply_relevance(left: Relevance, right: Relevance) -> Relevance {
    Relevance::new(left.probability * right.probability, left.strength * right.strength)
}

fn compare_relevance_desc(left: Relevance, right: Relevance) -> Ordering {
    right
        .probability
        .partial_cmp(&left.probability)
        .unwrap_or(Ordering::Equal)
        .then_with(|| right.strength.partial_cmp(&left.strength).unwrap_or(Ordering::Equal))
}

fn compare_canonical_relevance_desc(left: Relevance, right: Relevance) -> Ordering {
    // Preserve the public 1.x/2.x relevance-map ordering above while using
    // 3.x-style magnitude ordering for canonical text output.
    right
        .probability
        .partial_cmp(&left.probability)
        .unwrap_or(Ordering::Equal)
        .then_with(|| right.strength.abs().partial_cmp(&left.strength.abs()).unwrap_or(Ordering::Equal))
        .then_with(|| right.strength.partial_cmp(&left.strength).unwrap_or(Ordering::Equal))
}

fn format_relevance_strength(relevance: Relevance) -> String {
    match relevance.strength {
        1.0 => String::new(),
        -1.0 => "!".to_owned(),
        strength => format!("x{}", format_float(strength)),
    }
}

fn format_relevance_probability(relevance: Relevance) -> String {
    format!("{}%", format_float(relevance.probability * 100.0))
}

fn format_float(value: f32) -> String {
    let out = value.to_string();
    out.strip_suffix(".0").unwrap_or(&out).to_owned()
}

#[cfg(test)]
mod research;

#[cfg(test)]
mod tests {
    use super::*;

    fn full_question_snapshot(pangine: &mut Pangine, percepts: &[ConceptId], question: &ConceptId) -> QuestionSnapshot {
        let sources = percepts
            .iter()
            .flat_map(|percept| {
                pangine
                    .percept_roots
                    .get(&percept.index())
                    .into_iter()
                    .flatten()
                    .map(|(root, &occurrences)| QuestionSource { percept: percept.clone(), root: root.clone(), occurrences })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let mut ordered_widths = BTreeSet::new();
        pangine.collect_ordered_question_widths(question, &mut BTreeSet::new(), &mut BTreeMap::new(), &mut ordered_widths);
        let mut snapshot = QuestionSnapshot::default();
        for source in sources {
            pangine.add_question_experience_rec(&source, &source.root, &ordered_widths, None, &mut BTreeSet::new(), &mut snapshot.experiences);
            Pangine::add_question_graph_rec(&source, &source.root, &mut BTreeSet::new(), &mut snapshot.source_concepts, &mut snapshot.graph);
        }
        snapshot
    }

    #[test]
    fn debug_console_help_covers_current_language_surface() {
        let help = debug_console_help("help").unwrap();
        assert_eq!(debug_console_help("h"), Some(help));
        assert_eq!(debug_console_help("[help]"), None);
        for expected in [
            "[]                         Null",
            "[A]/[B]",
            "50%x2[A]x3[B]              Relevance",
            "['name'] ~= expression     Experience",
            "['source'] @ expression    Ask one Percept",
            "['a']['b'] @ expression   Ask several Percepts together",
            "$['*']                     Inspect all live ordinary concepts",
            "increments its occurrence count",
            "^['choice']",
        ] {
            assert!(help.contains(expected), "missing help entry: {expected}");
        }
    }

    #[test]
    fn debug_console_commands_are_exact() {
        assert!(debug_console_quit("q"));
        assert!(debug_console_quit("quit"));
        assert!(!debug_console_quit("query"));
        assert!(!debug_console_quit("quitting"));
    }

    #[test]
    fn dropping_pangine_releases_percept_value_graphs() {
        let weak_value = {
            let mut pangine = Pangine::new();
            let percept = pangine.reference_percept("memory");
            let value = pangine.reference_concept("['memory'][A]").unwrap().unwrap();
            let weak_value = Rc::downgrade(&value.0);

            assert!(pangine.set_percept_value(&percept, Some(value.clone())));
            drop(value);
            assert!(weak_value.upgrade().is_some());
            weak_value
        };

        assert!(weak_value.upgrade().is_none());
    }

    #[test]
    fn incremental_experience_materialization_matches_a_full_root_rebuild() {
        let mut pangine = Pangine::new();
        let percept = pangine.reference_percept("memory");
        let atomic = pangine.reference_concept("[A]").unwrap().unwrap();
        let inverse = pangine.reference_concept("![A]").unwrap().unwrap();
        let pair = pangine.reference_concept("[A][B]").unwrap().unwrap();
        let weighted_pair = pangine.reference_concept("x2[A][B]").unwrap().unwrap();
        let sequence = [weighted_pair.clone(), atomic, pair, inverse, weighted_pair];

        for (step, root) in sequence.into_iter().enumerate() {
            let root_text = pangine.format_concept(&root, false);
            let value = pangine.perform_experience(&percept, Some(&root));
            assert_eq!(value, pangine.get_value(&percept));

            let roots = pangine.percept_roots[&percept.index()].clone();
            let rebuilt = pangine.experience_value_map(&roots);
            assert_eq!(pangine.percept_value_maps[&percept.index()], rebuilt, "after step {step} experiencing {root_text}");
        }

        let previous = pangine.get_value(&percept).unwrap();
        let previous_text = pangine.format_concept(&previous, false);
        pangine.percept_value_maps.remove(&percept.index());
        let final_root = pangine.reference_concept("[C]->[D]").unwrap().unwrap();
        pangine.perform_experience(&percept, Some(&final_root));
        let roots = pangine.percept_roots[&percept.index()].clone();
        let rebuilt = pangine.experience_value_map(&roots);
        assert_eq!(pangine.percept_value_maps[&percept.index()], rebuilt);
        assert_eq!(pangine.format_concept(&previous, false), previous_text);

        let current_text = pangine.format_concept(&pangine.get_value(&percept).unwrap(), false);
        pangine.percept_value_maps.remove(&percept.index());
        pangine.percept_values.remove(&percept.index());
        let restored = pangine.materialize_percept_value(&percept).unwrap();
        assert_eq!(pangine.format_concept(&restored, false), current_text);
    }

    #[test]
    fn question_snapshot_drops_only_work_the_question_cannot_use() {
        let mut pangine = Pangine::new();
        for root in [
            "[C]->[bridge]->[E]",
            "[C]->[sound]->[quiet]",
            "[E]->[sound]->[loud]",
            "[C]*[sound]*[calm]",
            "[C]50%x2[bridge]",
            "[bridge]![E]",
            "{[C]->{[A]->[Z]}}",
        ] {
            let command = format!("['world'] ~= {root}");
            assert!(pangine.reference_concept(&command).unwrap().is_some());
        }

        let source = pangine.reference_percept("world");
        for question_text in ["[C]*[sound]*['unordered-answer']", "[C]->[sound]->['ordered-answer']", "{['who']->{[A]->[Z]}}", "['anything']"] {
            let question = pangine.reference_concept(question_text).unwrap().unwrap();
            let filtered = pangine.question_snapshot(std::slice::from_ref(&source), &question);
            let full = full_question_snapshot(&mut pangine, std::slice::from_ref(&source), &question);
            assert!(filtered.experiences.is_subset(&full.experiences));
            assert!(filtered.source_concepts.is_subset(&full.source_concepts));

            let filtered_results = pangine.get_projection_results(&question, &filtered);
            let full_results = pangine.get_projection_results(&question, &full);
            assert_eq!(filtered_results, full_results, "question {question_text}");
        }

        let unordered = pangine.reference_concept("[C]*[sound]*['unordered-answer']").unwrap().unwrap();
        let unordered_snapshot = pangine.question_snapshot(std::slice::from_ref(&source), &unordered);
        assert!(unordered_snapshot.experiences.iter().all(|experience| experience.matched.0.shape() == ConceptShape::Unordered));
        assert!(unordered_snapshot.source_concepts.is_empty());
        assert!(unordered_snapshot.graph.steps.is_empty());

        let contextual = pangine.reference_concept("[C]->[sound]->['ordered-answer']").unwrap().unwrap();
        let contextual_snapshot = pangine.question_snapshot(std::slice::from_ref(&source), &contextual);
        assert!(contextual_snapshot.experiences.iter().all(|experience| experience.matched.0.shape() == ConceptShape::Ordered(3)));
        assert!(!contextual_snapshot.source_concepts.is_empty());
        assert!(!contextual_snapshot.graph.steps.is_empty());

        let wildcard = pangine.reference_percept("anything");
        let wildcard_snapshot = pangine.question_snapshot(std::slice::from_ref(&source), &wildcard);
        let full_wildcard = full_question_snapshot(&mut pangine, std::slice::from_ref(&source), &wildcard);
        assert!(wildcard_snapshot.experiences == full_wildcard.experiences);
        assert!(wildcard_snapshot.source_concepts.is_empty());
        assert!(wildcard_snapshot.graph.steps.is_empty());
    }
}

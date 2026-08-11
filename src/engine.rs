use crate::Relevance;
use std::cell::Cell;
use std::cmp::Ordering;
use std::collections::{hash_map::DefaultHasher, BTreeMap, BTreeSet};
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::{self, Write};
use std::path::Path;
use std::rc::{Rc, Weak};
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

mod completion;
mod concept_map;

pub use completion::{Completion, CompletionEvidence, CompletionRemainder, CompletionRemainderSide, CompletionResult};
use concept_map::ConceptMap;

type CompositeLookup = BTreeMap<u64, Vec<Weak<Concept>>>;
type ExperienceRoots = BTreeMap<ConceptId, u64>;
type ProjectionAssignment = BTreeMap<ConceptId, ConceptId>;
// One exact experienced root supports a compatibility-view candidate according
// to that root's occurrence count. Recursive views and completion rows leading
// to the same answer within that root collapse here.
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
struct QuestionExperience {
    source: QuestionSource,
    matched: ConceptId,
}

#[derive(Default)]
struct QuestionSnapshot {
    experiences: BTreeSet<QuestionExperience>,
}

static NEXT_PANGINE_ID: AtomicUsize = AtomicUsize::new(0);

/// The reserved name of the global percept.
pub const GLOBAL_PERCEPT_NAME: &str = "*";

const DEBUG_CONSOLE_HELP: &str = "\
Commands:
  help, h        Show this help
  quit, q        Exit

Concept syntax:
  []                         Null / no Concept
  [name]                     Named Concept
  ['name']                   Percept reference
  (expression)               Make one complete surrounding operand
  [A][B]                     Union
  [A]*[B]                    Merge unordered Concept members
  [A]/[B]                    Merge with inverted [B]
  ![A]                       Inversion
  [A]->[B]->[C]              Ordered composition
  x2[A]x3[B]                 Signed coefficients

Percept operations:
  ['name'] = expression      Assign
  ['name'] += expression     Union addition
  ['name'] -= expression     Union subtraction
  ['name'] *= expression     Merge unordered Concept members
  ['name'] /= expression     Inverse merge
  ['name'] ~= expression     Experience
  ['source'] @ expression    Complete one source; return rows and bind holes
  ['a']['b'] @ expression   Complete several selected sources together
  $operand                   Recursively evaluate every Percept in the operand
  $['*']                     Inspect all live ordinary Concepts

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
  greatest positive current weight.

  ['choice'] = x2[tea]x3[coffee]
  ^['choice']             returns [coffee]

  Exact top-weight ties use the earliest canonical Concept spelling. If no
  entry has positive weight, ^ returns []. Zero-weight entries disappear when
  their Concept is built and are not decision candidates. This is a
  deterministic baseline rule. Richer sampling behavior remains open.
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
    // Local accelerator for the existing weak canonical registry. Complete
    // equality, not the fingerprint, still determines Concept identity.
    composite_lookup: CompositeLookup,
    // Rebuild the weak indexes only as their stored size grows geometrically.
    next_index_prune_size: usize,
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
            composite_lookup: CompositeLookup::new(),
            next_index_prune_size: 2,
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
        self.parse_statement_text(script)
    }

    /// Parses and executes every statement in a script string.
    pub fn parse_script_text(&mut self, script: &str) -> ParseResult<Option<ConceptId>> {
        self.parse_script_text_impl(script, None)
    }

    /// Parses a script string while writing each statement and result to `details`.
    pub fn parse_script_text_with_details<W: Write>(&mut self, script: &str, details: &mut W) -> ParseResult<Option<ConceptId>> {
        self.parse_script_text_impl(script, Some(details))
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

    /// Explicitly merges `merge` into a mutable percept and returns its updated value.
    pub fn perform_merge(&mut self, percept: &ConceptId, merge: Option<&ConceptId>) -> Option<ConceptId> {
        if !self.accepts_percept_input(percept, merge) {
            return None;
        }

        let value = self.perform_merge_update(percept, merge.cloned(), false);
        self.set_percept_value(percept, value.clone());
        value
    }

    /// Explicitly merges the inverse of `merge` into a mutable percept and returns its updated value.
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

    /// Returns entries ordered by descending `x`, then canonical Concept order.
    ///
    /// An unordered composition returns its member edges. Any other Concept is
    /// treated as a single default-coefficient entry.
    pub fn get_relevance_map(&self, concept: &ConceptId) -> Vec<(Relevance, ConceptId)> {
        let mut map = self.relevance_entries(concept).unwrap_or_default();

        map.sort_by(|(left_rel, left_concept), (right_rel, right_concept)| {
            compare_coefficients_desc(*left_rel, *right_rel).then_with(|| self.compare_concepts(left_concept, right_concept))
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
        let Some(concept) = concept.filter(|concept| self.owns(concept)) else {
            return vec!["  []".to_owned()];
        };

        let entries = self.get_relevance_map(concept);
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
        let mut operands = Vec::new();

        if let Some(operand) = self.parse_union_operand(parser)? {
            operands.push(operand);
        }

        loop {
            parser.skip_ws();
            if !parser.starts_union_operand() {
                break;
            }

            if let Some(operand) = self.parse_union_operand(parser)? {
                operands.push(operand);
            }
        }

        Ok(self.reference_union(&operands))
    }

    fn parse_union_operand(&mut self, parser: &mut Parser) -> ParseResult<Option<ParsedUnionOperand>> {
        parser.skip_ws();

        if parser.consume('x') {
            let x_coefficient = parser.parse_float();
            let mut operand = self.parse_union_operand(parser)?.ok_or(ParseError::InvalidSyntax)?;
            operand.relevance = multiply_relevance(Relevance::new(x_coefficient), operand.relevance);
            return Ok(Some(operand));
        }

        match parser.peek() {
            Some('(') => {
                parser.next();
                let concept = self.parse_expression(parser)?;
                parser.expect(')')?;
                Ok(concept.map(ParsedUnionOperand::ordinary))
            }
            Some('[') => Ok(self.parse_bracket(parser)?.map(ParsedUnionOperand::ordinary)),
            Some('{') => Ok(self.parse_ordered(parser)?.map(ParsedUnionOperand::ordinary)),
            Some('$') => {
                parser.next();
                let operand = self.parse_union_operand(parser)?.ok_or(ParseError::InvalidSyntax)?;
                let operand = self.reference_union(&[operand]).ok_or(ParseError::InvalidSyntax)?;
                Ok(self.evaluate_concept(&operand).map(ParsedUnionOperand::ordinary))
            }
            Some('^') => {
                parser.next();
                let operand = self.parse_union_operand(parser)?.ok_or(ParseError::InvalidSyntax)?;
                let operand = self.reference_union(&[operand]).ok_or(ParseError::InvalidSyntax)?;
                Ok(self.make_decision(&operand).map(ParsedUnionOperand::ordinary))
            }
            Some('!') => {
                parser.next();
                parser.skip_ws();
                let concept_start = parser.pos;
                let mut operand = self.parse_union_operand(parser)?;
                if operand.is_none() && parser.pos == concept_start {
                    return Err(ParseError::InvalidSyntax);
                }
                if let Some(operand) = operand.as_mut() {
                    operand.relevance.x_coefficient = -operand.relevance.x_coefficient;
                }
                Ok(operand)
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
        self.maybe_prune_indexes();
        Some(concept)
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

    fn reference_union(&mut self, operands: &[ParsedUnionOperand]) -> Option<ConceptId> {
        let mut map = ConceptMap::new();

        for operand in operands {
            self.add_union_concept(&mut map, operand.concept.clone(), false, operand.relevance);
        }

        self.reference_map(&map)
    }

    fn experience_value_map(&mut self, roots: &ExperienceRoots) -> ConceptMap {
        let mut map = ConceptMap::new();
        for (root, &count) in roots {
            self.add_union_concept(&mut map, root.clone(), false, Relevance::new(count as f32));
        }
        map
    }

    fn sole_default_concept(map: &ConceptMap) -> Option<&ConceptId> {
        let (concept, relevance) = map.first_key_value()?;
        (map.len() == 1 && *relevance == Relevance::DEFAULT).then_some(concept)
    }

    fn reference_map(&mut self, map: &ConceptMap) -> Option<ConceptId> {
        if map.is_empty() {
            return None;
        }

        // 3.x returns a sole default-coefficient Concept directly before interning:
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
        let fingerprint = Self::composite_fingerprint(&kind, &subconcepts);
        if let Some(bucket) = self.composite_lookup.get_mut(&fingerprint) {
            let mut existing = None;
            bucket.retain(|candidate| {
                let Some(candidate) = candidate.upgrade() else {
                    return false;
                };
                if existing.is_none() && candidate.kind == kind && candidate.subconcepts == subconcepts {
                    existing = Some(ConceptId(candidate));
                }
                true
            });
            if let Some(existing) = existing {
                return existing;
            }
        }

        let concept = self.alloc(kind, subconcepts);
        let weak = Rc::downgrade(&concept.0);
        self.composites.push(weak.clone());
        self.composite_lookup.entry(fingerprint).or_default().push(weak);
        self.maybe_prune_indexes();
        concept
    }

    fn composite_fingerprint(kind: &ConceptKind, subconcepts: &ConceptMap) -> u64 {
        let mut hasher = DefaultHasher::new();
        match kind {
            ConceptKind::Named(name) => {
                0_u8.hash(&mut hasher);
                name.hash(&mut hasher);
            }
            ConceptKind::Percept { name } => {
                1_u8.hash(&mut hasher);
                name.hash(&mut hasher);
            }
            ConceptKind::Unordered => 2_u8.hash(&mut hasher),
            ConceptKind::Ordered { components } => {
                3_u8.hash(&mut hasher);
                components.hash(&mut hasher);
            }
        }
        subconcepts.hash_lookup_summary(&mut hasher);
        hasher.finish()
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
        self.composite_lookup.clear();
        for concept in self.composites.iter().filter_map(Weak::upgrade) {
            let fingerprint = Self::composite_fingerprint(&concept.kind, &concept.subconcepts);
            self.composite_lookup.entry(fingerprint).or_default().push(Rc::downgrade(&concept));
        }

        let live_size = self.names.len().saturating_add(self.composites.len());
        self.next_index_prune_size = live_size.checked_next_power_of_two().and_then(|size| size.checked_mul(2)).unwrap_or(usize::MAX).max(2);
    }

    fn maybe_prune_indexes(&mut self) {
        if self.names.len().saturating_add(self.composites.len()) >= self.next_index_prune_size {
            self.prune_indexes();
        }
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
        Some((value.clone(), value))
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

    fn answer_question(&mut self, percepts: &[ConceptId], question: Option<ConceptId>) -> Option<ConceptId> {
        let question = question?;
        let result = self.complete_question(percepts, &question)?;
        let rows = self.materialize_completion_rows(&result);
        let projection_results = self.materialize_completion_bindings(&result);

        for (percept, binding_result) in projection_results {
            self.set_percept_value(&percept, binding_result);
        }

        rows
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
            experiences.insert(QuestionExperience { source: source.clone(), matched: concept.clone() });
        }

        match &concept.0.kind {
            ConceptKind::Ordered { components } => {
                let components = components.clone();
                for &width in ordered_widths.range(2..components.len()) {
                    if experience_shapes.is_some_and(|shapes| !shapes.contains(&ConceptShape::Ordered(width))) {
                        continue;
                    }
                    for window in components.windows(width) {
                        let matched = self.reference_ordered(window.to_vec());
                        experiences.insert(QuestionExperience { source: source.clone(), matched });
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
                    let coefficient_concept =
                        if relevance == Relevance::DEFAULT { Some(child) } else { self.reference_map(&ConceptMap::from([(child, relevance)])) };
                    if let Some(coefficient_concept) = coefficient_concept {
                        self.add_question_experience_rec(source, &coefficient_concept, ordered_widths, experience_shapes, visited, experiences);
                    }
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
    fn materialize_question_witnesses(
        &mut self,
        output_percepts: BTreeSet<ConceptId>,
        mut witnesses: QuestionCandidateWitnesses,
    ) -> BTreeMap<ConceptId, Option<ConceptId>> {
        let mut results = BTreeMap::new();
        for output in output_percepts {
            let mut candidates = ConceptMap::new();
            for (candidate, candidate_witnesses) in witnesses.remove(&output).unwrap_or_default() {
                // The output x coefficient is a support count for deterministic
                // choice, not a wider judgment about the candidate.
                let support = candidate_witnesses.iter().map(|source| u128::from(source.occurrences)).sum::<u128>();
                self.add_relevance(&mut candidates, candidate, false, Relevance::new(support as f32));
            }
            results.insert(output, self.reference_map(&candidates));
        }
        results
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

    fn add_relevance(&mut self, map: &mut ConceptMap, concept: ConceptId, inversion: bool, mut relevance: Relevance) {
        if inversion {
            relevance.x_coefficient = -relevance.x_coefficient;
        }

        if map.contains_key(&concept) {
            let mut current = map[&concept];
            current.add(relevance);
            if current.is_empty() {
                map.remove(&concept);
            } else {
                map.insert(concept, current);
            }
        } else if !relevance.is_empty() {
            map.insert(concept, relevance);
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
            compare_canonical_coefficients_desc(*left_relevance, *right_relevance).then_with(|| self.compare_concepts(left_concept, right_concept))
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
            let order = compare_canonical_coefficients_desc(left_relevance, right_relevance);
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
            out.push_str(&format_x_coefficient(relevance));
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
        let add_separator = relevance.x_coefficient != 1.0 && relevance.x_coefficient != -1.0;
        let wrap_concept = relevance.x_coefficient != 1.0 && matches!(concept.0.kind, ConceptKind::Unordered);

        if relevance.x_coefficient == -1.0 {
            out.push('!');
        }

        if relevance.x_coefficient != 1.0 && relevance.x_coefficient != -1.0 {
            out.push_str(&format_x_coefficient(relevance));
        }

        if add_separator && !wrap_concept {
            out.push(' ');
        }

        if wrap_concept {
            out.push('(');
        }
        out.push_str(&self.format_concept(concept, false));
        if wrap_concept {
            out.push(')');
        }
        out
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

    fn parse_float(&mut self) -> f32 {
        self.parse_number().unwrap_or(0.0)
    }

    fn starts_union_operand(&mut self) -> bool {
        self.peek().is_some_and(|c| matches!(c, '(' | '[' | '{' | '$' | '^' | '!' | 'x'))
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
    Relevance::new(left.x_coefficient * right.x_coefficient)
}

fn compare_coefficients_desc(left: Relevance, right: Relevance) -> Ordering {
    right.x_coefficient.partial_cmp(&left.x_coefficient).unwrap_or(Ordering::Equal)
}

fn compare_canonical_coefficients_desc(left: Relevance, right: Relevance) -> Ordering {
    // Canonical text groups larger magnitudes first while retaining the sign
    // as a deterministic tie-breaker.
    right
        .x_coefficient
        .abs()
        .partial_cmp(&left.x_coefficient.abs())
        .unwrap_or(Ordering::Equal)
        .then_with(|| right.x_coefficient.partial_cmp(&left.x_coefficient).unwrap_or(Ordering::Equal))
}

fn format_x_coefficient(relevance: Relevance) -> String {
    match relevance.x_coefficient {
        1.0 => String::new(),
        -1.0 => "!".to_owned(),
        x_coefficient => format!("x{}", format_float(x_coefficient)),
    }
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
            "(expression)               Make one complete surrounding operand",
            "[A]*[B]                    Merge unordered Concept members",
            "[A]/[B]",
            "x2[A]x3[B]                 Signed coefficients",
            "['name'] ~= expression     Experience",
            "['source'] @ expression    Complete one source",
            "['a']['b'] @ expression   Complete several selected sources together",
            "$['*']                     Inspect all live ordinary Concepts",
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
    fn composite_lookup_treats_signed_zero_coefficients_as_equal() {
        let mut pangine = Pangine::new();
        let member = pangine.reference_named("member").unwrap();
        let positive_zero = ConceptMap::from([(member.clone(), Relevance::new(0.0))]);
        let negative_zero = ConceptMap::from([(member, Relevance::new(-0.0))]);

        assert_eq!(positive_zero, negative_zero);
        assert_eq!(
            Pangine::composite_fingerprint(&ConceptKind::Unordered, &positive_zero),
            Pangine::composite_fingerprint(&ConceptKind::Unordered, &negative_zero)
        );
        let first = pangine.reference_composite(ConceptKind::Unordered, positive_zero);
        let second = pangine.reference_composite(ConceptKind::Unordered, negative_zero);
        assert_eq!(first, second);
    }

    #[test]
    fn incremental_experience_materialization_matches_a_full_root_rebuild() {
        let mut pangine = Pangine::new();
        let percept = pangine.reference_percept("memory");
        let atomic = pangine.reference_concept("[A]").unwrap().unwrap();
        let inverse = pangine.reference_concept("![A]").unwrap().unwrap();
        let pair = pangine.reference_concept("[A][B]").unwrap().unwrap();
        let coefficient_pair = pangine.reference_concept("x2[A][B]").unwrap().unwrap();
        let sequence = [coefficient_pair.clone(), atomic, pair, inverse, coefficient_pair];

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
    fn every_retained_experience_return_keeps_its_original_value_and_identity() {
        let mut pangine = Pangine::new();
        let percept = pangine.reference_percept("memory");
        let roots = (0..64)
            .map(|index| {
                let item = pangine.reference_named(&format!("item-{index}")).unwrap();
                let answer = pangine.reference_named(&format!("answer-{index}")).unwrap();
                pangine.reference_ordered(vec![item, answer])
            })
            .collect::<Vec<_>>();
        let mut returns = Vec::with_capacity(roots.len());

        for root in &roots {
            pangine.record_experience(&percept, root).unwrap();
            returns.push(pangine.materialize_percept_value(&percept).unwrap());
        }

        for (index, returned) in returns.iter().enumerate() {
            let expected = roots[..=index].iter().cloned().map(|root| (root, Relevance::DEFAULT)).collect::<ConceptMap>();
            let reconstructed = pangine.reference_map(&expected).unwrap();
            assert_eq!(&reconstructed, returned);
            assert_eq!(pangine.format_concept(&reconstructed, false), pangine.format_concept(returned, false));
        }
    }

    #[test]
    fn question_snapshot_drops_only_work_the_question_cannot_use() {
        let mut pangine = Pangine::new();
        for root in
            ["[C]->[bridge]->[E]", "[C]->[sound]->[quiet]", "[E]->[sound]->[loud]", "[C]*[sound]*[calm]", "[C]x2[bridge]", "[bridge]![E]", "{[C]->{[A]->[Z]}}"]
        {
            let command = format!("['world'] ~= {root}");
            assert!(pangine.reference_concept(&command).unwrap().is_some());
        }

        let source = pangine.reference_percept("world");
        for question_text in ["[C]*[sound]*['unordered-answer']", "[C]->[sound]->['ordered-answer']", "{['who']->{[A]->[Z]}}", "['anything']"] {
            let question = pangine.reference_concept(question_text).unwrap().unwrap();
            let filtered = pangine.question_snapshot(std::slice::from_ref(&source), &question);
            let full = full_question_snapshot(&mut pangine, std::slice::from_ref(&source), &question);
            assert!(filtered.experiences.is_subset(&full.experiences));

            let filtered_results = pangine.complete_question_snapshot(&question, &filtered);
            let full_results = pangine.complete_question_snapshot(&question, &full);
            assert!(filtered_results.completions() == full_results.completions(), "question {question_text}");
        }

        let unordered = pangine.reference_concept("[C]*[sound]*['unordered-answer']").unwrap().unwrap();
        let unordered_snapshot = pangine.question_snapshot(std::slice::from_ref(&source), &unordered);
        assert!(unordered_snapshot.experiences.iter().all(|experience| experience.matched.0.shape() == ConceptShape::Unordered));

        let ordered = pangine.reference_concept("[C]->[sound]->['ordered-answer']").unwrap().unwrap();
        let ordered_snapshot = pangine.question_snapshot(std::slice::from_ref(&source), &ordered);
        assert!(ordered_snapshot.experiences.iter().all(|experience| experience.matched.0.shape() == ConceptShape::Ordered(3)));

        let wildcard = pangine.reference_percept("anything");
        let wildcard_snapshot = pangine.question_snapshot(std::slice::from_ref(&source), &wildcard);
        let full_wildcard = full_question_snapshot(&mut pangine, std::slice::from_ref(&source), &wildcard);
        assert!(wildcard_snapshot.experiences == full_wildcard.experiences);
    }
}

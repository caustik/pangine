use crate::Relevance;
use std::cell::Cell;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::{self, Write};
use std::path::Path;
use std::rc::{Rc, Weak};
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

type ConceptMap = BTreeMap<ConceptId, Relevance>;
type BindingMap = BTreeMap<ConceptId, ConceptMap>;
type ProjectionBindingWeights = BTreeMap<ConceptId, BTreeMap<ConceptId, f64>>;
type ProjectionCache = BTreeMap<(usize, usize), ProjectionSummary>;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum RelationKind {
    Dependency,
    Correlation,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ConceptShape {
    Named,
    Percept,
    Unordered,
    Relation(RelationKind),
}

#[derive(Clone)]
struct ProjectionSummary {
    // Total folded projection mass, including paths that bind no output.
    total: f64,
    // First-order output marginals. Distinct output percepts do not require
    // materializing their joint Cartesian product.
    bindings: ProjectionBindingWeights,
}

impl ProjectionSummary {
    fn wildcard() -> Self {
        Self { total: 1.0, bindings: ProjectionBindingWeights::new() }
    }

    fn variable(percept: ConceptId, candidate: ConceptId) -> Self {
        Self {
            // Wildcarding this node leaves the output unbound; preserving it
            // binds the exact experienced subtree.
            total: 2.0,
            bindings: ProjectionBindingWeights::from([(percept, BTreeMap::from([(candidate, 1.0)]))]),
        }
    }

    fn multiply(&self, other: &Self) -> Self {
        let mut product = Self { total: self.total * other.total, bindings: ProjectionBindingWeights::new() };
        product.accumulate_bindings(self, other.total);
        product.accumulate_bindings(other, self.total);
        product
    }

    fn add(&mut self, other: Self) {
        self.total += other.total;
        self.accumulate_bindings(&other, 1.0);
    }

    fn accumulate_bindings(&mut self, other: &Self, scale: f64) {
        for (percept, candidates) in &other.bindings {
            for (candidate, weight) in candidates {
                *self.bindings.entry(percept.clone()).or_default().entry(candidate.clone()).or_default() += weight * scale;
            }
        }
    }

    fn scale(&mut self, scale: f64) {
        self.total *= scale;
        for candidates in self.bindings.values_mut() {
            for weight in candidates.values_mut() {
                *weight *= scale;
            }
        }
    }
}

static NEXT_PANGINE_ID: AtomicUsize = AtomicUsize::new(0);

/// The reserved name of the global percept.
pub const GLOBAL_PERCEPT_NAME: &str = "*";

const DEBUG_CONSOLE_HELP: &str = "\
Commands:
  help, h        Show this help
  quit, q        Exit
  @<expression>  Print the result with percepts evaluated

Concept syntax:
  []                         Null / no concept
  [name]                     Named concept
  ['name']                   Percept reference
  [?name]                    Question-namespaced concept, not a binding
  (expression)               Grouping
  [A][B]                     Union
  [A]*[B]                    Flattening merge
  [A]/[B]                    Merge with inverted [B]
  ![A]                       Inversion
  [A]->[B]                   Correlation
  ?[A]:[B]                   Dependency
  <50%x2[A], [B]>            Relevance

Percept operations:
  ['name'] = expression      Assign
  ['name'] += expression     Union addition
  ['name'] -= expression     Union subtraction
  ['name'] *= expression     Flattening merge
  ['name'] /= expression     Inverse merge
  ['name'] ~= expression     Experience
  ['name'] @ expression      Bind outputs; return the question shape
  $operand                   Recursively evaluate every percept in the operand
  $['*']                     Snapshot all live ordinary concepts

Experience:
  ['memory'] ~= {[cat]->[purrs]}
  Stores exact recursive structure and accumulated relevance. Questions fold
  the implied recursive wildcard projections lazily instead of storing them.

Scripts:
  expression; expression    Multiple statements
  // line comment            C++-style comment
  /* block comment */        C-style comment

Decision:
  ^['choice'] evaluates the percept and returns the entry with the greatest
  positive probability * strength.

  ['choice'] = <x2[tea], x3[coffee]>
  ^['choice']             returns [coffee]

  Ties currently use allocation order. If no entry has positive weight, the
  complete evaluated value is returned.
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
    /// An anonymous unordered concept.
    Anonymous,
    /// A directed correlation from `a` to `b`.
    Correlation {
        /// The source concept.
        a: ConceptId,
        /// The target concept.
        b: ConceptId,
    },
    /// A dependency whose question shape is `a` and answer shape is `b`.
    Dependency {
        /// The question concept.
        a: ConceptId,
        /// The answer concept.
        b: ConceptId,
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
            ConceptKind::Anonymous => ConceptShape::Unordered,
            ConceptKind::Correlation { .. } => ConceptShape::Relation(RelationKind::Correlation),
            ConceptKind::Dependency { .. } => ConceptShape::Relation(RelationKind::Dependency),
        }
    }

    fn relation(&self) -> Option<(RelationKind, &ConceptId, &ConceptId)> {
        match &self.kind {
            ConceptKind::Correlation { a, b } => Some((RelationKind::Correlation, a, b)),
            ConceptKind::Dependency { a, b } => Some((RelationKind::Dependency, a, b)),
            _ => None,
        }
    }

    fn children(&self) -> impl Iterator<Item = (&ConceptId, Relevance)> {
        let relation = match self.relation() {
            Some((_, a, b)) => [Some(a), Some(b)],
            None => [None, None],
        };

        relation.into_iter().flatten().map(|child| (child, Relevance::DEFAULT)).chain(self.subconcepts.iter().map(|(child, &relevance)| (child, relevance)))
    }
}

/// A deterministic concept engine with isolated identity and percept state.
pub struct Pangine {
    id: usize,
    next_concept_id: Cell<usize>,
    names: BTreeMap<String, Weak<Concept>>,
    percepts: BTreeMap<String, ConceptId>,
    percept_values: BTreeMap<usize, ConceptId>,
    anon: Vec<Weak<Concept>>,
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
            percept_values: BTreeMap::new(),
            anon: Vec::new(),
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
        self.reference_concept_with_params(script, &[])
    }

    /// Parses and executes Pangine syntax with positional concept parameters.
    pub fn reference_concept_with_params(&mut self, script: &str, params: &[ConceptId]) -> ParseResult<Option<ConceptId>> {
        if params.iter().any(|concept| !self.owns(concept)) {
            return Err(ParseError::InvalidSyntax);
        }

        let result = self.parse_statement_text_with_params(script, params);
        self.prune_indexes();
        result
    }

    /// Converts the names in plain text into an unordered concept.
    pub fn parse_text(&mut self, text: &str) -> Option<ConceptId> {
        let mut map = ConceptMap::new();

        for name in text.split(|c| !is_name_char(c, false)).filter(|name| !name.is_empty()) {
            if let Some(concept) = self.reference_named(name) {
                self.add_relevance(&mut map, concept, false, Relevance::DEFAULT);
            }
        }

        self.reference_map(&map)
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

        self.perform_union_update(percept, addition.cloned(), false).and_then(|(value, _)| value)
    }

    /// Subtracts `subtraction` from a mutable percept and returns its updated value.
    pub fn perform_subtraction(&mut self, percept: &ConceptId, subtraction: Option<&ConceptId>) -> Option<ConceptId> {
        if !self.accepts_percept_input(percept, subtraction) {
            return None;
        }

        self.perform_union_update(percept, subtraction.cloned(), true).and_then(|(value, _)| value)
    }

    /// Flattens `merge` into a mutable percept and returns its updated value.
    pub fn perform_merge(&mut self, percept: &ConceptId, merge: Option<&ConceptId>) -> Option<ConceptId> {
        if !self.accepts_percept_input(percept, merge) {
            return None;
        }

        self.perform_merge_update(percept, merge.cloned(), false)
    }

    /// Flattens the inverse of `merge` into a mutable percept and returns its updated value.
    pub fn perform_inverse_merge(&mut self, percept: &ConceptId, merge: Option<&ConceptId>) -> Option<ConceptId> {
        if !self.accepts_percept_input(percept, merge) {
            return None;
        }

        self.perform_merge_update(percept, merge.cloned(), true)
    }

    /// Accumulates an experience in a mutable percept and returns the resulting concept.
    pub fn perform_experience(&mut self, percept: &ConceptId, experience: Option<&ConceptId>) -> Option<ConceptId> {
        if !self.accepts_percept_input(percept, experience) {
            return None;
        }

        let mut map = self.percept_value_map(percept)?;

        if let Some(experience) = experience {
            self.add_relevance_rec(&mut map, experience.clone(), false, Relevance::DEFAULT);
        }

        self.reference_map(&map)
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

    /// Replaces a mutable percept's value, returning whether the input was valid.
    pub fn set_percept_value(&mut self, percept: &ConceptId, value: Option<ConceptId>) -> bool {
        if !self.is_mutable_percept(percept) || value.as_ref().is_some_and(|concept| !self.owns(concept)) {
            return false;
        }

        match value {
            Some(value) => self.percept_values.insert(percept.index(), value),
            None => self.percept_values.remove(&percept.index()),
        };
        true
    }

    /// Returns the source of an owned correlation.
    pub fn get_correlation_a(&self, concept: &ConceptId) -> Option<ConceptId> {
        self.correlation(concept).map(|(a, _)| a.clone())
    }

    /// Returns the target of an owned correlation.
    pub fn get_correlation_b(&self, concept: &ConceptId) -> Option<ConceptId> {
        self.correlation(concept).map(|(_, b)| b.clone())
    }

    /// Returns the question side of an owned dependency.
    pub fn get_dependency_a(&self, concept: &ConceptId) -> Option<ConceptId> {
        self.dependency(concept).map(|(a, _)| a.clone())
    }

    /// Returns the answer side of an owned dependency.
    pub fn get_dependency_b(&self, concept: &ConceptId) -> Option<ConceptId> {
        self.dependency(concept).map(|(_, b)| b.clone())
    }

    /// Returns `concept` when it is an owned percept.
    pub fn get_percept(&self, concept: &ConceptId) -> Option<ConceptId> {
        self.is_percept(concept).then(|| concept.clone())
    }

    /// Returns a correlation's source when it is a percept.
    pub fn get_percept_a(&self, concept: &ConceptId) -> Option<ConceptId> {
        self.get_correlation_a(concept).filter(|concept| self.is_percept(concept))
    }

    /// Returns a correlation's target when it is a percept.
    pub fn get_percept_b(&self, concept: &ConceptId) -> Option<ConceptId> {
        self.get_correlation_b(concept).filter(|concept| self.is_percept(concept))
    }

    /// Returns relevance entries ordered by descending weight and concept identity.
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
    pub fn debug_console_lines(&self, concept: Option<&ConceptId>, evaluate: bool) -> Vec<String> {
        // Historical anchor:
        // 1.x/pangine/src/pangine/common/pae_pangine.cpp:1311
        let Some(entries) = concept.and_then(|concept| self.relevance_entries(concept)) else {
            return vec!["  []".to_owned()];
        };

        entries.into_iter().map(|(relevance, concept)| self.format_debug_console_line(relevance, &concept, evaluate)).collect()
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

            let command = input.trim_end_matches(['\r', '\n']);
            let (evaluate, script) = command.strip_prefix('@').map_or((false, command), |script| (true, script));

            if script.starts_with('q') {
                break;
            }

            if let Some(help) = debug_console_help(script) {
                print!("{help}");
                continue;
            }

            match self.reference_concept(script) {
                Ok(concept) => {
                    for line in self.debug_console_lines(concept.as_ref(), evaluate) {
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
    // 3.x semantics are right-associative and bind below union and merge:
    // 3.x/pangine/include/pangine/pae_concept_parser.h:69,84,102
    fn parse_expression(&mut self, parser: &mut Parser) -> ParseResult<Option<ConceptId>> {
        let concept = self.parse_merge_expression(parser)?;

        parser.skip_ws();
        if !parser.consume_str("->") {
            return Ok(concept);
        }

        let right = self.parse_expression(parser)?;
        match (concept, right) {
            (Some(left), Some(right)) => Ok(Some(self.reference_correlation(left, right))),
            _ => Err(ParseError::InvalidSyntax),
        }
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
        self.parse_statement_text_with_params(script, &[])
    }

    fn parse_statement_text_with_params(&mut self, script: &str, params: &[ConceptId]) -> ParseResult<Option<ConceptId>> {
        let mut parser = Parser::new(script, params);
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
            if !parser.peek().is_some_and(starts_union_operand) {
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

        match parser.peek() {
            Some('(') => {
                parser.next();
                let concept = self.parse_expression(parser)?;
                parser.expect(')')?;
                Ok(concept)
            }
            Some('[') => self.parse_bracket(parser),
            Some('{') => self.parse_correlation(parser),
            Some('<') => self.parse_relevance(parser),
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
            Some('?') => self.parse_dependency(parser),
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

    fn parse_correlation(&mut self, parser: &mut Parser) -> ParseResult<Option<ConceptId>> {
        parser.next();
        let left = self.parse_merge_expression(parser)?.ok_or(ParseError::InvalidSyntax)?;
        parser.expect('-')?;
        parser.expect('>')?;
        let right = self.parse_merge_expression(parser)?.ok_or(ParseError::InvalidSyntax)?;
        parser.expect('}')?;
        Ok(Some(self.reference_correlation(left, right)))
    }

    fn parse_dependency(&mut self, parser: &mut Parser) -> ParseResult<Option<ConceptId>> {
        parser.next();
        let dependency = self.parse_expression(parser)?.ok_or(ParseError::InvalidSyntax)?;
        parser.expect(':')?;
        let consequent = self.parse_expression(parser)?.ok_or(ParseError::InvalidSyntax)?;
        Ok(Some(self.reference_dependency(dependency, consequent)))
    }

    fn parse_relevance(&mut self, parser: &mut Parser) -> ParseResult<Option<ConceptId>> {
        parser.next();
        let mut map = ConceptMap::new();

        loop {
            let probability = parser.parse_probability();

            let mut strength = 1.0;
            if parser.consume('x') {
                strength = parser.parse_float();
            }

            if let Some(term) = self.parse_expression(parser)? {
                self.add_union_concept(&mut map, term, false, Relevance::new(probability, strength));
            }

            if parser.consume(',') {
                parser.skip_ws();
                continue;
            }

            parser.expect('>')?;
            break;
        }

        Ok(self.reference_map(&map))
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

        if parser.consume('%') {
            let concept = parser.params.pop_front();
            parser.expect(']')?;
            return Ok(concept);
        }

        let is_question = parser.consume('?');
        parser.consume('&');

        let mut name = parser.parse_name(true);
        if is_question {
            name.insert(0, '?');
        }

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
            Question,
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
        } else if parser.consume('@') {
            Action::Question
        } else {
            return Ok(Some(percept));
        };

        if self.is_global_percept(&percept) && !matches!(action, Action::Question) {
            return Err(ParseError::InvalidSyntax);
        }

        parser.skip_ws();
        let input = self.parse_expression(parser)?;
        let value = match action {
            Action::Assign => input,
            Action::Add => {
                let Some((value, stored)) = self.perform_union_update(&percept, input, false) else {
                    return Ok(None);
                };
                self.set_percept_value(&percept, stored);
                return Ok(value);
            }
            Action::Subtract => {
                let Some((value, stored)) = self.perform_union_update(&percept, input, true) else {
                    return Ok(None);
                };
                self.set_percept_value(&percept, stored);
                return Ok(value);
            }
            Action::Merge => self.perform_merge(&percept, input.as_ref()),
            Action::InverseMerge => self.perform_inverse_merge(&percept, input.as_ref()),
            Action::Experience => self.perform_experience(&percept, input.as_ref()),
            Action::Question => return Ok(self.answer_question(&percept, input)),
        };
        self.set_percept_value(&percept, value.clone());
        Ok(value)
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

        let existing = self
            .anon
            .iter()
            .filter_map(Weak::upgrade)
            .map(ConceptId)
            .filter(|concept| matches!(concept.0.kind, ConceptKind::Anonymous))
            .find(|concept| Self::compare_subconcepts(concept, map, true));
        if let Some(concept) = existing {
            return Some(concept);
        }

        let concept = self.alloc(ConceptKind::Anonymous, map.clone());
        self.anon.push(Rc::downgrade(&concept.0));
        Some(concept)
    }

    fn reference_correlation(&mut self, a: ConceptId, b: ConceptId) -> ConceptId {
        self.reference_pair(ConceptKind::Correlation { a, b })
    }

    fn reference_dependency(&mut self, a: ConceptId, b: ConceptId) -> ConceptId {
        self.reference_pair(ConceptKind::Dependency { a, b })
    }

    fn reference_relation(&mut self, kind: RelationKind, a: ConceptId, b: ConceptId) -> ConceptId {
        match kind {
            RelationKind::Dependency => self.reference_dependency(a, b),
            RelationKind::Correlation => self.reference_correlation(a, b),
        }
    }

    fn reference_pair(&mut self, kind: ConceptKind) -> ConceptId {
        self.prune_indexes();

        for concept in self.anon.iter().filter_map(Weak::upgrade) {
            if concept.kind == kind {
                return ConceptId(concept);
            }
        }

        let concept = self.alloc(kind, ConceptMap::new());
        self.anon.push(Rc::downgrade(&concept.0));
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
        self.names.values().chain(&self.anon).filter_map(Weak::upgrade).map(ConceptId)
    }

    fn global_value(&self) -> Option<ConceptId> {
        let map = self.live_ordinary_concepts().map(|concept| (concept, Relevance::DEFAULT)).collect::<ConceptMap>();

        self.reference_transient_map(map)
    }

    fn reference_transient_map(&self, map: ConceptMap) -> Option<ConceptId> {
        if map.is_empty() {
            return None;
        }

        if let Some(concept) = Self::sole_default_concept(&map) {
            return Some(concept.clone());
        }

        Some(self.alloc(ConceptKind::Anonymous, map))
    }

    fn prune_indexes(&mut self) {
        self.names.retain(|_, concept| concept.strong_count() > 0);
        self.anon.retain(|concept| concept.strong_count() > 0);
    }

    fn compare_subconcepts(concept: &ConceptId, map: &ConceptMap, compare_relevance: bool) -> bool {
        if map.is_empty() {
            return false;
        }

        if Self::sole_default_concept(map) == Some(concept) {
            return true;
        }

        let subconcepts = &concept.0.subconcepts;
        if !compare_relevance {
            return map.len() == subconcepts.len() && map.keys().all(|concept| subconcepts.contains_key(concept));
        }

        map == subconcepts
    }
}

// Percept updates and recursive evaluation.
impl Pangine {
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
        let stored = self.percept_union_stored_value(&map, value.clone());
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

        let map = current.0.subconcepts.clone();
        if !map.is_empty() {
            return Some(map);
        }

        let mut map = ConceptMap::new();
        self.add_union_concept(&mut map, current, false, Relevance::DEFAULT);
        Some(map)
    }

    fn percept_union_stored_value(&mut self, map: &ConceptMap, value: Option<ConceptId>) -> Option<ConceptId> {
        if let Some(concept) = Self::sole_default_concept(map) {
            if !concept.0.subconcepts.is_empty() {
                let stored = self.alloc(ConceptKind::Anonymous, map.clone());
                self.anon.push(Rc::downgrade(&stored.0));
                return Some(stored);
            }
        }

        value
    }

    fn answer_question(&mut self, percept: &ConceptId, question: Option<ConceptId>) -> Option<ConceptId> {
        let question = question?;
        let Some(value) = self.get_value(percept) else {
            return Some(question);
        };
        let mut experiences = value.0.subconcepts.clone();
        if experiences.is_empty() {
            experiences.insert(value, Relevance::DEFAULT);
        }

        let projection_results = self.get_projection_results(&question, &experiences);

        for (percept, binding_result) in projection_results {
            self.set_percept_value(&percept, binding_result);
        }

        Some(question)
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
            ConceptKind::Anonymous => {
                let evaluated = self.evaluate_subconcepts(concept, visited_percepts);
                self.reference_map(&evaluated)
            }
            ConceptKind::Correlation { .. } | ConceptKind::Dependency { .. } => {
                let (kind, a, b) = concept.0.relation().unwrap();
                let (a, b) = (a.clone(), b.clone());
                let a = self.evaluate_concept_inner(&a, visited_percepts)?;
                let b = self.evaluate_concept_inner(&b, visited_percepts)?;
                Some(self.reference_relation(kind, a, b))
            }
        }
    }

    fn evaluate_transient_concept(&mut self, concept: &ConceptId, visited_percepts: &mut BTreeSet<ConceptId>) -> Option<ConceptId> {
        if matches!(concept.0.kind, ConceptKind::Anonymous) {
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
    fn get_projection_results(&mut self, question: &ConceptId, experiences: &ConceptMap) -> BTreeMap<ConceptId, Option<ConceptId>> {
        let mut questions = ConceptMap::new();
        let mut contains_percept_cache = BTreeMap::new();
        self.collect_question_patterns(question, Relevance::DEFAULT, true, &mut questions, &mut contains_percept_cache);

        let mut output_percepts = BTreeSet::new();
        self.collect_output_percepts(question, &mut output_percepts);
        let mut bindings = output_percepts.into_iter().map(|percept| (percept, ConceptMap::new())).collect::<BindingMap>();
        let mut cache = ProjectionCache::new();
        let mut experience_index = BTreeMap::<ConceptShape, Vec<_>>::new();
        for experience in experiences {
            let (concept, _) = experience;
            experience_index.entry(concept.0.shape()).or_default().push(experience);
        }

        for (question, &question_relevance) in &questions {
            let matching_experiences = if self.is_percept(question) {
                experiences.iter().collect::<Vec<_>>()
            } else {
                experience_index.get(&question.0.shape()).into_iter().flatten().copied().collect::<Vec<_>>()
            };

            for (experience, &experience_relevance) in matching_experiences {
                let summary = self.projection_summary(experience, question, &mut cache);
                for (percept, candidates) in summary.bindings {
                    for (candidate, weight) in candidates {
                        let relevance = projection_relevance(experience_relevance, question_relevance, weight);
                        self.add_relevance(bindings.entry(percept.clone()).or_default(), candidate, false, relevance);
                    }
                }
            }
        }

        bindings.into_iter().map(|(percept, candidates)| (percept, self.reference_map(&candidates))).collect()
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
        relevance: Relevance,
        is_root: bool,
        patterns: &mut ConceptMap,
        contains_percept_cache: &mut BTreeMap<usize, bool>,
    ) {
        if !self.contains_percept(question, contains_percept_cache) {
            return;
        }

        if is_root || !self.is_percept(question) {
            patterns.entry(question.clone()).and_modify(|current| current.add(relevance)).or_insert(relevance);
        }

        for (child, child_relevance) in question.0.children() {
            self.collect_question_patterns(child, multiply_relevance(relevance, child_relevance), false, patterns, contains_percept_cache);
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

    fn projection_summary(&self, experience: &ConceptId, question: &ConceptId, cache: &mut ProjectionCache) -> ProjectionSummary {
        let key = (experience.index(), question.index());
        if let Some(summary) = cache.get(&key) {
            return summary.clone();
        }

        if self.is_percept(question) {
            let summary = ProjectionSummary::variable(question.clone(), experience.clone());
            cache.insert(key, summary.clone());
            return summary;
        }

        let mut summary = ProjectionSummary::wildcard();
        let preserved = if let (ConceptKind::Named(experience_name), ConceptKind::Named(question_name)) = (&experience.0.kind, &question.0.kind) {
            (experience_name == question_name).then(ProjectionSummary::wildcard)
        } else if let (Some((experience_kind, experience_a, experience_b)), Some((question_kind, question_a, question_b))) =
            (experience.0.relation(), question.0.relation())
        {
            if experience_kind != question_kind {
                None
            } else {
                let a = self.projection_summary(experience_a, question_a, cache);
                let b = self.projection_summary(experience_b, question_b, cache);
                Some(a.multiply(&b))
            }
        } else if experience.0.shape() == ConceptShape::Unordered && question.0.shape() == ConceptShape::Unordered {
            self.unordered_projection_summary(experience, question, cache)
        } else {
            None
        };

        if let Some(preserved) = preserved {
            summary.add(preserved);
        }
        cache.insert(key, summary.clone());
        summary
    }

    fn unordered_projection_summary(&self, experience: &ConceptId, question: &ConceptId, cache: &mut ProjectionCache) -> Option<ProjectionSummary> {
        let experiences = experience.0.subconcepts.iter().collect::<Vec<_>>();
        let questions = question.0.subconcepts.iter().collect::<Vec<_>>();
        if experiences.len() != questions.len() {
            return None;
        }

        let state_count = 1usize.checked_shl(experiences.len() as u32)?;
        let edges = questions
            .iter()
            .map(|(question, question_relevance)| {
                experiences
                    .iter()
                    .map(|(experience, experience_relevance)| {
                        let mut edge = self.projection_summary(experience, question, cache);
                        edge.scale((experience_relevance.weight() * question_relevance.weight()) as f64);
                        edge
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();

        let mut forward = vec![0.0; state_count];
        forward[0] = 1.0;
        // This subset DP computes the weighted permanent without enumerating
        // every child assignment.
        for mask in 0..state_count {
            let question_index = mask.count_ones() as usize;
            if question_index == questions.len() {
                continue;
            }

            for (experience_index, edge) in edges[question_index].iter().enumerate() {
                let bit = 1usize << experience_index;
                if mask & bit == 0 {
                    forward[mask | bit] += forward[mask] * edge.total;
                }
            }
        }

        let mut reverse = vec![0.0; state_count];
        let mut derivatives = vec![vec![0.0; experiences.len()]; questions.len()];
        reverse[state_count - 1] = 1.0;
        // Reverse-mode derivatives give every edge's contribution to the
        // permanent, which turns edge-local bindings into output marginals.
        for mask in (0..state_count - 1).rev() {
            let question_index = mask.count_ones() as usize;
            for (experience_index, edge) in edges[question_index].iter().enumerate() {
                let bit = 1usize << experience_index;
                if mask & bit != 0 {
                    continue;
                }

                let next = mask | bit;
                reverse[mask] += reverse[next] * edge.total;
                derivatives[question_index][experience_index] += reverse[next] * forward[mask];
            }
        }

        let mut summary = ProjectionSummary { total: forward[state_count - 1], bindings: ProjectionBindingWeights::new() };
        for (question_index, row) in edges.into_iter().enumerate() {
            for (experience_index, edge) in row.into_iter().enumerate() {
                let derivative = derivatives[question_index][experience_index];
                summary.accumulate_bindings(&edge, derivative);
            }
        }

        Some(summary)
    }
}

// Relevance accumulation and structural access.
impl Pangine {
    fn add_merge_concept(&mut self, map: &mut ConceptMap, concept: ConceptId, inversion: bool, relevance: Relevance) {
        let subconcepts = concept.0.subconcepts.clone();
        if !self.is_percept(&concept) && !subconcepts.is_empty() {
            for (child, child_relevance) in subconcepts {
                self.add_union_concept(map, child, inversion, multiply_relevance(relevance, child_relevance));
            }
        } else {
            self.add_union_concept(map, concept, inversion, relevance);
        }
    }

    fn add_union_concept(&mut self, map: &mut ConceptMap, concept: ConceptId, inversion: bool, relevance: Relevance) {
        let subconcepts = concept.0.subconcepts.clone();
        if !self.is_percept(&concept) && subconcepts.len() == 1 {
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
        let found = map.keys().cloned().find_map(|candidate| {
            if candidate == concept {
                Some((candidate, false))
            } else if Self::compare_subconcepts(&candidate, &concept_subconcepts, false) {
                Some((candidate, true))
            } else {
                None
            }
        });

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

    fn add_relevance_rec(&mut self, map: &mut ConceptMap, concept: ConceptId, inversion: bool, relevance: Relevance) {
        let subconcepts = concept.0.subconcepts.clone();
        let relation = concept.0.relation().map(|(_, a, b)| [a.clone(), b.clone()]);

        if subconcepts.len() != 1 {
            self.add_relevance(map, concept, inversion, relevance);
        }

        for child in relation.into_iter().flatten() {
            self.add_relevance_rec(map, child, inversion, relevance);
        }

        for (child, child_relevance) in subconcepts {
            self.add_relevance_rec(map, child, inversion, child_relevance);
        }
    }

    fn make_decision(&self, concept: &ConceptId) -> Option<ConceptId> {
        let mut concept = self.get_value(concept)?;
        let subconcepts = concept.0.subconcepts.clone();

        if subconcepts.is_empty() {
            return Some(concept);
        }

        let mut greatest = 0.0;
        for (candidate, relevance) in subconcepts {
            let current = relevance.weight();
            if current > greatest {
                greatest = current;
                concept = candidate;
            }
        }

        Some(concept)
    }

    fn correlation<'a>(&self, concept: &'a ConceptId) -> Option<(&'a ConceptId, &'a ConceptId)> {
        self.relation(concept, RelationKind::Correlation)
    }

    fn dependency<'a>(&self, concept: &'a ConceptId) -> Option<(&'a ConceptId, &'a ConceptId)> {
        self.relation(concept, RelationKind::Dependency)
    }

    fn relation<'a>(&self, concept: &'a ConceptId, expected: RelationKind) -> Option<(&'a ConceptId, &'a ConceptId)> {
        if !self.owns(concept) {
            return None;
        }

        concept.0.relation().and_then(|(kind, a, b)| (kind == expected).then_some((a, b)))
    }

    fn relevance_entries(&self, concept: &ConceptId) -> Option<Vec<(Relevance, ConceptId)>> {
        if !self.owns(concept) {
            return None;
        }

        Some(if concept.0.subconcepts.is_empty() {
            vec![(Relevance::DEFAULT, concept.clone())]
        } else {
            concept.0.subconcepts.iter().map(|(concept, &relevance)| (relevance, concept.clone())).collect()
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
            ConceptKind::Correlation { a, b } => {
                let a = self.format_semantic_operand(a, evaluate, active);
                let b = self.format_semantic_operand(b, evaluate, active);
                format!("{{{a}->{b}}}")
            }
            ConceptKind::Dependency { a, b } => {
                let a_paren = self.needs_dependency_parens(a);
                // Historical 1.x used dependency A to decide parentheses on both sides:
                // 1.x/pangine/src/pangine/common/pae_concept.cpp:157
                let b_paren = self.needs_dependency_parens(a);
                let a = self.format_inner(a, evaluate, active);
                let b = self.format_inner(b, evaluate, active);
                format!(
                    "?{}{}{}:{}{}{}",
                    if a_paren { "(" } else { "" },
                    a,
                    if a_paren { ")" } else { "" },
                    if b_paren { "(" } else { "" },
                    b,
                    if b_paren { ")" } else { "" }
                )
            }
            ConceptKind::Anonymous => self.format_subconcepts(&concept.0.subconcepts, evaluate, active, true),
        };

        active.remove(concept);
        formatted
    }

    fn needs_dependency_parens(&self, concept: &ConceptId) -> bool {
        concept.0.shape() == ConceptShape::Relation(RelationKind::Dependency) || concept.0.subconcepts.len() > 1
    }

    fn format_semantic_operand(&self, concept: &ConceptId, evaluate: bool, active: &mut BTreeSet<ConceptId>) -> String {
        if matches!(concept.0.kind, ConceptKind::Anonymous) {
            let map = &concept.0.subconcepts;
            if Self::can_format_as_implicit_union(map) {
                return self.format_subconcepts(map, evaluate, active, false);
            }
        }

        self.format_inner(concept, evaluate, active)
    }

    fn can_format_as_implicit_union(map: &ConceptMap) -> bool {
        map.len() > 1 && map.values().all(|relevance| relevance.probability == 1.0 && (relevance.strength == 1.0 || relevance.strength == -1.0))
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

        match (left.0.relation(), right.0.relation()) {
            (Some((left_kind, left_a, left_b)), Some((right_kind, right_a, right_b))) => {
                let order =
                    left_kind.cmp(&right_kind).then_with(|| self.compare_concepts(left_a, right_a)).then_with(|| self.compare_concepts(left_b, right_b));
                if order != Ordering::Equal {
                    return order;
                }
            }
            (Some(_), None) => return Ordering::Greater,
            (None, Some(_)) => return Ordering::Less,
            (None, None) => {}
        }

        left.cmp(right)
    }

    fn format_subconcepts(&self, map: &ConceptMap, evaluate: bool, active: &mut BTreeSet<ConceptId>, wrap_implicit_union: bool) -> String {
        let use_implicit_union = Self::can_format_as_implicit_union(map);
        let use_relevance = !use_implicit_union
            && (map.len() > 1 || map.values().any(|relevance| relevance.probability != 1.0 || (relevance.strength != 1.0 && relevance.strength != -1.0)));
        let mut out = String::new();

        for (index, (concept, relevance)) in self.canonical_entries(map).into_iter().enumerate() {
            if use_implicit_union {
                if wrap_implicit_union && index == 0 {
                    out.push('(');
                }
            } else if use_relevance {
                if index == 0 {
                    out.push('<');
                } else {
                    out.push_str(", ");
                }

                if relevance.probability != 1.0 {
                    out.push_str(&format_relevance_probability(relevance));
                }
            }

            out.push_str(&format_relevance_strength(relevance));
            let wrap_dependency = use_implicit_union && concept.0.shape() == ConceptShape::Relation(RelationKind::Dependency);
            if wrap_dependency {
                out.push('(');
            }
            out.push_str(&self.format_inner(&concept, evaluate, active));
            if wrap_dependency {
                out.push(')');
            }
        }

        if use_implicit_union && wrap_implicit_union {
            out.push(')');
        } else if use_relevance {
            out.push('>');
        }

        out
    }

    fn format_debug_console_line(&self, relevance: Relevance, concept: &ConceptId, evaluate: bool) -> String {
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

        out.push_str(&self.format_concept(concept, evaluate));
        out
    }
}

struct Parser {
    chars: Vec<char>,
    pos: usize,
    params: VecDeque<ConceptId>,
}

impl Parser {
    fn new(script: &str, params: &[ConceptId]) -> Self {
        Self { chars: script.chars().collect(), pos: 0, params: params.iter().cloned().collect() }
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
                (Some('#'), _) | (Some('/'), Some('/')) => self.skip_line_comment(),
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

fn starts_union_operand(c: char) -> bool {
    matches!(c, '(' | '[' | '{' | '<' | '$' | '^' | '?' | '!')
}

fn debug_console_help(command: &str) -> Option<&'static str> {
    matches!(command, "h" | "help").then_some(DEBUG_CONSOLE_HELP)
}

fn statement_has_tokens(statement: &str) -> bool {
    let mut parser = Parser::new(statement, &[]);
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
                    statements.push(&script[start..index]);
                    start = index + ch.len_utf8();
                }
            }
            continue;
        }

        match ch {
            '#' => in_line_comment = true,
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
            '<' => stack.push('>'),
            ')' | ']' | '}' | '>' if stack.last() == Some(&ch) => {
                stack.pop();
            }
            _ => {}
        }
    }

    statements.push(&script[start..]);
    ScriptStatements { items: statements, has_semicolons }
}

fn multiply_relevance(left: Relevance, right: Relevance) -> Relevance {
    Relevance::new(left.probability * right.probability, left.strength * right.strength)
}

fn projection_relevance(experience: Relevance, question: Relevance, score: f64) -> Relevance {
    Relevance::new(experience.probability * question.probability, experience.strength * question.strength * score as f32)
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
mod tests {
    use super::*;

    #[test]
    fn debug_console_help_covers_current_language_surface() {
        let help = debug_console_help("help").unwrap();
        assert_eq!(debug_console_help("h"), Some(help));
        assert_eq!(debug_console_help("[help]"), None);
        assert!(help.contains("help, h"));
        assert!(help.contains("[]                         Null"));
        assert!(help.contains("[A]/[B]"));
        assert!(help.contains("?[A]:[B]"));
        assert!(help.contains("['name'] += expression"));
        assert!(help.contains("['name'] -= expression"));
        assert!(help.contains("['name'] *= expression"));
        assert!(help.contains("['name'] /= expression"));
        assert!(help.contains("['name'] ~= expression     Experience"));
        assert!(help.contains("['name'] @ expression      Bind outputs; return the question shape"));
        assert!(help.contains("$operand                   Recursively evaluate every percept in the operand"));
        assert!(help.contains("Stores exact recursive structure"));
        assert!(help.contains("wildcard projections lazily"));
        assert!(help.contains("expression; expression"));
        assert!(help.contains("^['choice']"));
        assert!(help.contains("probability * strength"));
        assert!(help.contains("allocation order"));
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
}

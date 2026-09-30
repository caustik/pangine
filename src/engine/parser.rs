//! Script entry points, statement splitting, lexical rules, and the recursive-descent parser.

use super::{ConceptId, Pangine, ParsedUnionOperand, GLOBAL_PERCEPT_NAME};
use crate::Relevance;
use std::fs;
use std::io::{self, Write};
use std::path::Path;

/// The result of parsing or executing Pangine syntax.
pub type ParseResult<T> = Result<T, ParseError>;

/// An error produced while parsing a script or reading a script file.
#[derive(Debug)]
#[non_exhaustive]
pub enum ParseError {
    /// The input does not conform to Pangine syntax.
    InvalidSyntax,
    /// A coefficient operation exceeded the signed 64-bit relevance range.
    RelevanceOverflow,
    /// A script or details file could not be read or written.
    Io(io::Error),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidSyntax => formatter.write_str("invalid Pangine syntax"),
            Self::RelevanceOverflow => formatter.write_str("relevance coefficient exceeds the signed 64-bit range"),
            Self::Io(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ParseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidSyntax | Self::RelevanceOverflow => None,
            Self::Io(error) => Some(error),
        }
    }
}

impl From<io::Error> for ParseError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

// Script entry points.
impl Pangine {
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

// Recursive-descent parser implementation.
impl Pangine {
    fn parse_expression(&mut self, parser: &mut Parser) -> ParseResult<Option<ConceptId>> {
        let selector = self.parse_ordered_expression(parser)?;

        parser.skip_ws();
        let adjustment_factor = if parser.consume_str("@+=") {
            Some(Relevance::DEFAULT)
        } else if parser.consume_str("@-=") {
            Some(Relevance::new(-1))
        } else {
            None
        };
        if let Some(factor) = adjustment_factor {
            let target = selector.ok_or(ParseError::InvalidSyntax)?;
            parser.skip_ws();
            let adjustment_start = parser.pos;
            let adjustment = self.parse_expression(parser)?;
            if parser.pos == adjustment_start {
                return Err(ParseError::InvalidSyntax);
            }

            let adjustment = adjustment.ok_or(ParseError::InvalidSyntax)?;
            let target_view = self.answer_view(&target).ok_or(ParseError::InvalidSyntax)?;
            let adjustment_view = self.answer_view(&adjustment).ok_or(ParseError::InvalidSyntax)?;
            let adjusted = target_view.adjusted_by(self, &adjustment_view, factor).ok_or(ParseError::InvalidSyntax)?;
            let published = adjusted.answer().publish(self).map_err(|_| ParseError::InvalidSyntax)?;
            let published_view = published.view(self, target).ok_or(ParseError::InvalidSyntax)?;
            return Ok(published_view.materialize(self));
        }

        if !parser.consume('@') {
            return Ok(selector);
        }

        let selector = selector.ok_or(ParseError::InvalidSyntax)?;
        let selector = self.question_selector(&selector);
        parser.skip_ws();
        let question_start = parser.pos;
        let question = self.parse_expression(parser)?;
        if parser.pos == question_start {
            return Err(ParseError::InvalidSyntax);
        }
        Ok(self.answer_question(selector, question))
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
            concept = self.reference_merge_with_inversion(concept, rhs, inversion)?;
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

        self.reference_union(&operands)
    }

    fn parse_union_operand(&mut self, parser: &mut Parser) -> ParseResult<Option<ParsedUnionOperand>> {
        parser.skip_ws();

        if parser.consume('x') {
            let x_coefficient = parser.parse_integer()?.ok_or(ParseError::InvalidSyntax)?;
            let mut operand = self.parse_union_operand(parser)?.ok_or(ParseError::InvalidSyntax)?;
            operand.relevance = Relevance::new(x_coefficient).checked_mul(operand.relevance).ok_or(ParseError::RelevanceOverflow)?;
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
            Some('{') => Ok(self.parse_percept(parser)?.map(ParsedUnionOperand::ordinary)),
            Some(operator @ ('$' | '&' | '^')) => {
                parser.next();
                let operand = self.parse_union_operand(parser)?.ok_or(ParseError::InvalidSyntax)?;
                let operand = self.reference_union(&[operand])?.ok_or(ParseError::InvalidSyntax)?;
                let result = match operator {
                    '$' => self.evaluate_concept(&operand),
                    '&' => self.linked_answer(&operand),
                    '^' => self.make_decision(&operand),
                    _ => unreachable!(),
                };
                Ok(result.map(ParsedUnionOperand::ordinary))
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
                    operand.relevance = operand.relevance.checked_neg().ok_or(ParseError::RelevanceOverflow)?;
                }
                Ok(operand)
            }
            _ => Ok(None),
        }
    }

    fn parse_percept(&mut self, parser: &mut Parser) -> ParseResult<Option<ConceptId>> {
        parser.next();
        let name = if parser.peek() == Some('"') {
            parser.parse_quoted_text()?
        } else if parser.consume('*') {
            GLOBAL_PERCEPT_NAME.to_owned()
        } else {
            let name = parser.parse_name(true);
            if name.is_empty() {
                return Err(ParseError::InvalidSyntax);
            }
            name
        };
        let percept = self.reference_percept(&name);
        parser.expect('}')?;

        parser.skip_ws();
        self.parse_percept_action(parser, percept)
    }

    fn parse_bracket(&mut self, parser: &mut Parser) -> ParseResult<Option<ConceptId>> {
        parser.next();

        let concept = if parser.peek() == Some('"') {
            let name = parser.parse_quoted_text()?;
            Some(self.reference_name(&name))
        } else {
            let name = parser.parse_name(true);
            self.reference_named(&name)
        };
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

    fn parse_quoted_text(&mut self) -> ParseResult<String> {
        self.expect('"')?;
        let mut text = String::new();

        loop {
            match self.next().ok_or(ParseError::InvalidSyntax)? {
                '"' => return Ok(text),
                '\\' => text.push(self.parse_text_escape()?),
                character if character.is_control() => return Err(ParseError::InvalidSyntax),
                character => text.push(character),
            }
        }
    }

    fn parse_text_escape(&mut self) -> ParseResult<char> {
        match self.next().ok_or(ParseError::InvalidSyntax)? {
            '"' => Ok('"'),
            '\\' => Ok('\\'),
            '0' => Ok('\0'),
            'b' => Ok('\u{0008}'),
            't' => Ok('\t'),
            'n' => Ok('\n'),
            'f' => Ok('\u{000c}'),
            'r' => Ok('\r'),
            'u' => self.parse_unicode_escape(),
            _ => Err(ParseError::InvalidSyntax),
        }
    }

    fn parse_unicode_escape(&mut self) -> ParseResult<char> {
        self.expect('{')?;

        let mut value = 0_u32;
        let mut digits = 0;
        while let Some(digit) = self.peek().and_then(|character| character.to_digit(16)) {
            if digits == 6 {
                return Err(ParseError::InvalidSyntax);
            }

            self.next();
            value = value.checked_mul(16).and_then(|current| current.checked_add(digit)).ok_or(ParseError::InvalidSyntax)?;
            digits += 1;
        }

        if digits == 0 {
            return Err(ParseError::InvalidSyntax);
        }

        self.expect('}')?;
        char::from_u32(value).ok_or(ParseError::InvalidSyntax)
    }

    fn starts_union_operand(&mut self) -> bool {
        self.peek().is_some_and(|c| matches!(c, '(' | '[' | '{' | '$' | '&' | '^' | '!' | 'x'))
    }

    fn parse_integer(&mut self) -> ParseResult<Option<i64>> {
        let start = self.pos;

        if self.peek() == Some('-') {
            self.pos += 1;
        }

        let mut has_digit = false;
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            has_digit = true;
            self.pos += 1;
        }

        if !has_digit {
            self.pos = start;
            return Ok(None);
        }

        let value = self.chars[start..self.pos].iter().collect::<String>().parse().map_err(|_| ParseError::RelevanceOverflow)?;

        Ok(Some(value))
    }
}

pub(super) fn is_name_char(c: char, allow_space: bool) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-' || (allow_space && c == ' ')
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
    let mut in_quoted_text = false;
    let mut quoted_escape = false;
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

        if in_quoted_text {
            if quoted_escape {
                quoted_escape = false;
            } else if ch == '\\' {
                quoted_escape = true;
            } else if ch == '"' {
                in_quoted_text = false;
            }
            continue;
        }

        match ch {
            '"' => in_quoted_text = true,
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

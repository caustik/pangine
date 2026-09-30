//! Canonical Concept ordering and formatting.

use super::{parser::is_name_char, ConceptId, ConceptKind, ConceptMap, Pangine, GLOBAL_PERCEPT_NAME};
use crate::Relevance;
use std::cmp::Ordering;
use std::collections::BTreeSet;

#[derive(Clone, Copy, PartialEq, Eq)]
enum FormatContext {
    Root,
    UnionMember,
    OrderedComponent,
}

// Canonical presentation and ordering.
impl Pangine {
    /// Formats an owned concept as canonical Pangine syntax.
    pub fn format_concept(&self, concept: &ConceptId, evaluate: bool) -> String {
        if !self.owns(concept) {
            return "[]".to_owned();
        }

        let mut active = BTreeSet::new();
        self.format_inner(concept, evaluate, &mut active, FormatContext::Root)
    }

    fn format_inner(&self, concept: &ConceptId, evaluate: bool, active: &mut BTreeSet<ConceptId>, context: FormatContext) -> String {
        if !active.insert(concept.clone()) {
            return match &concept.0.kind {
                ConceptKind::Named(name) => self.format_named(name),
                ConceptKind::Percept { name } => self.format_percept(name),
                _ => format!("[#{}]", concept.index()),
            };
        }

        let formatted = match &concept.0.kind {
            ConceptKind::Named(name) => self.format_named(name),
            ConceptKind::Percept { name } => {
                if evaluate {
                    self.get_value(concept).map_or_else(|| "[]".to_owned(), |value| self.format_inner(&value, evaluate, active, context))
                } else {
                    self.format_percept(name)
                }
            }
            ConceptKind::Ordered { components } => {
                let mut ordered = String::new();
                for (index, component) in components.iter().enumerate() {
                    if index > 0 {
                        ordered.push_str("->");
                    }
                    ordered.push_str(&self.format_inner(component, evaluate, active, FormatContext::OrderedComponent));
                }
                if context == FormatContext::Root {
                    ordered
                } else {
                    format!("({ordered})")
                }
            }
            ConceptKind::Unordered => {
                let unordered = self.format_relevance(&concept.0.subconcepts, evaluate, active);
                if context == FormatContext::UnionMember {
                    format!("({unordered})")
                } else {
                    unordered
                }
            }
        };

        active.remove(concept);
        formatted
    }

    fn format_named(&self, name: &str) -> String {
        self.format_name(name, '[', ']', false)
    }

    fn format_percept(&self, name: &str) -> String {
        self.format_name(name, '{', '}', name == GLOBAL_PERCEPT_NAME)
    }

    fn format_name(&self, name: &str, opening: char, closing: char, reserved_compact: bool) -> String {
        if reserved_compact || (!name.is_empty() && name.chars().all(|character| is_name_char(character, true))) {
            return format!("{opening}{name}{closing}");
        }

        let mut formatted = String::new();
        formatted.push(opening);
        formatted.push('"');
        for character in name.chars() {
            match character {
                '"' => formatted.push_str("\\\""),
                '\\' => formatted.push_str("\\\\"),
                '\0' => formatted.push_str("\\0"),
                '\u{0008}' => formatted.push_str("\\b"),
                '\t' => formatted.push_str("\\t"),
                '\n' => formatted.push_str("\\n"),
                '\u{000c}' => formatted.push_str("\\f"),
                '\r' => formatted.push_str("\\r"),
                character if character.is_control() => formatted.push_str(&format!("\\u{{{:x}}}", u32::from(character))),
                character => formatted.push(character),
            }
        }
        formatted.push('"');
        formatted.push(closing);
        formatted
    }

    pub(super) fn canonical_entries(&self, map: &ConceptMap) -> Vec<(ConceptId, Relevance)> {
        let mut entries: Vec<_> = map.iter().map(|(concept, &relevance)| (concept.clone(), relevance)).collect();

        entries.sort_by(|(left_concept, left_relevance), (right_concept, right_relevance)| {
            compare_canonical_coefficients_desc(*left_relevance, *right_relevance).then_with(|| self.compare_concepts(left_concept, right_concept))
        });
        entries
    }

    // 3.x orders concepts by percept/name, union shape, relevance, and semantic
    // components rather than allocation order:
    // 3.x/pangine/src/libpangine/common/pae_concept.cpp:15
    pub(super) fn compare_concepts(&self, left: &ConceptId, right: &ConceptId) -> Ordering {
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
            out.push_str(&self.format_inner(&concept, evaluate, active, FormatContext::UnionMember));
        }

        out
    }
}

fn compare_canonical_coefficients_desc(left: Relevance, right: Relevance) -> Ordering {
    // Canonical text groups larger magnitudes first while retaining the sign
    // as a deterministic tie-breaker.
    right.x_coefficient.unsigned_abs().cmp(&left.x_coefficient.unsigned_abs()).then_with(|| right.x_coefficient.cmp(&left.x_coefficient))
}

pub(super) fn format_x_coefficient(relevance: Relevance) -> String {
    match relevance.x_coefficient {
        1 => String::new(),
        -1 => "!".to_owned(),
        x_coefficient => format!("x{x_coefficient}"),
    }
}

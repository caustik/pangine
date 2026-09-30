//! The interactive console, its help text, and diagnostic output lines.

use super::{format::format_x_coefficient, ConceptId, ConceptKind, Pangine};
use crate::Relevance;
use std::io::{self, Write};

const DEBUG_CONSOLE_HELP: &str = "\
Commands:
  help, h          Show this help
  inspect operand  Show linked values and the sources behind their strengths
  quit, q          Exit

Concept syntax:
  []                         No Concept
  [name]                     Named Concept
  [\"escaped text\"]           Named Concept with escaped text
  {name}                     Percept reference
  {\"escaped text\"}           Percept reference with escaped text
  (expression)               Make one complete surrounding operand
  [A][B]                     Union
  [A]*[B]                    Merge unordered Concept members
  [A]/[B]                    Merge with inverted [B]
  ![A]                       Inversion
  [A]->[B]->[C]              Ordered composition
  x2[A]x3[B]                 Signed integer coefficients

Percept operations:
  {name} = expression      Assign
  {name} += expression     Union addition
  {name} -= expression     Union subtraction
  {name} *= expression     Merge unordered Concept members
  {name} /= expression     Inverse merge
  {name} ~= expression     Capture one experience
  subject @ expression       Complete a Concept; return rows and bind holes
  {source} @ expression    Complete one retained Percept source
  {*} @ expression         Complete the global Percept's Concepts
  {a}{b} @ expression   Complete several retained sources together
  A structural subject keeps embedded Percepts as data. Use $ to evaluate them.
  &operand                   Return the shared answer shape for linked Percepts
  $operand                   Read Percepts without changing their shared answer
  {target} @+= {evidence} Add matching evidence to a linked Answer
  {target} @-= {evidence} Subtract matching evidence from a linked Answer
  ${*}                     Inspect all live ordinary Concepts

Experience:
  {input} = [purrs]
  {memory} ~= [cat]->{input}
  Evaluates assigned Percepts in the complete input, then records the grounded
  result as one experience owned by {memory}. Percepts populated by experience
  remain references. Repeating an equal Concept adds default relevance to that
  member. Questions derive recursive matches without multiplying one experience
  by match routes.

Scripts:
  expression; expression    Multiple statements
  // line comment            C++-style comment
  /* block comment */        C-style comment

Choice:
  ^operand chooses the greatest positive current result. For output Percepts
  from one question, it removes incompatible answers and refreshes every linked
  output. Several output Percepts in one operand are chosen together.

  {choice} = x2[tea]x3[coffee]
  ^{choice}             returns [coffee]
  ^({animal}->{food}) chooses one complete animal-food pair

  Exact top-weight ties use the earliest canonical Concept spelling. If no
  entry has positive weight, ^ returns []. Zero-weight entries disappear when
  their Concept is built and are not decision candidates. This is a
  deterministic baseline rule. Richer sampling behavior remains open.
";

// Interactive console and diagnostic lines.
impl Pangine {
    /// Formats relevance entries as individual debug-console lines.
    pub fn debug_console_lines(&self, concept: Option<&ConceptId>) -> Vec<String> {
        // Historical anchor:
        // 1.x/pangine/src/pangine/common/pae_pangine.cpp:1311
        let Some(concept) = concept.filter(|concept| self.owns(concept)) else {
            return vec!["  []".to_owned()];
        };

        // A raw Percept reference remains a reference in console presentation;
        // `$` is still the explicit value evaluation operation.
        let entries = self.sorted_relevance_entries(self.relevance_entries(concept).unwrap_or_default());
        entries.into_iter().map(|(relevance, concept)| self.format_debug_console_line(relevance, &concept)).collect()
    }

    fn debug_answer_inspection_lines(&mut self, operand: &str) -> Result<Vec<String>, String> {
        if operand.is_empty() {
            return Err("inspect expects one linked Answer operand".to_owned());
        }

        let concept =
            self.reference_concept(operand).map_err(|error| error.to_string())?.ok_or_else(|| "inspect expects one linked Answer operand".to_owned())?;
        let answer = self.answer_view(&concept).ok_or_else(|| "operand is not part of one linked Answer".to_owned())?;
        let possibilities = answer.possibilities(self).ok_or_else(|| "linked Answer could not be inspected".to_owned())?;
        if possibilities.is_empty() {
            return Ok(vec!["  no possibilities".to_owned()]);
        }

        let mut lines = Vec::new();
        for possibility in possibilities {
            let marker = if possibility.is_top_tie() { '*' } else { ' ' };
            let rows = possibility.complete_rows();
            let row_label = if rows == 1 { "row" } else { "rows" };
            let value = self.format_concept(possibility.value(), false);
            lines.push(format!("  {marker} {:+}, {rows} {row_label}: {value}", possibility.strength().weight()));

            let mut sources = possibility
                .sources()
                .iter()
                .map(|source| (self.format_concept(source.subject(), false), self.format_concept(source.concept(), false), source.contribution().weight()))
                .collect::<Vec<_>>();
            sources.sort();

            for (subject, source, contribution) in sources {
                lines.push(format!("      {contribution:+} from {subject}: {source}"));
            }
        }
        Ok(lines)
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

            if let Some(operand) = debug_console_inspection_operand(script) {
                match self.debug_answer_inspection_lines(operand) {
                    Ok(lines) => {
                        for line in lines {
                            println!("{line}");
                        }
                    }
                    Err(error) => println!("  {error}"),
                }
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

    fn format_debug_console_line(&self, relevance: Relevance, concept: &ConceptId) -> String {
        let mut out = String::from("  ");
        let add_separator = relevance.x_coefficient != 1 && relevance.x_coefficient != -1;
        let wrap_concept = relevance.x_coefficient != 1 && matches!(concept.0.kind, ConceptKind::Unordered | ConceptKind::Ordered { .. });

        if relevance.x_coefficient == -1 {
            out.push('!');
        }

        if relevance.x_coefficient != 1 && relevance.x_coefficient != -1 {
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

fn debug_console_help(command: &str) -> Option<&'static str> {
    matches!(command, "h" | "help").then_some(DEBUG_CONSOLE_HELP)
}

fn debug_console_inspection_operand(command: &str) -> Option<&str> {
    let operand = command.strip_prefix("inspect")?;
    if operand.is_empty() {
        return Some(operand);
    }
    operand.chars().next().is_some_and(char::is_whitespace).then(|| operand.trim_start())
}

fn debug_console_quit(command: &str) -> bool {
    matches!(command, "q" | "quit")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_console_help_covers_current_language_surface() {
        let help = debug_console_help("help").unwrap();
        assert_eq!(debug_console_help("h"), Some(help));
        assert_eq!(debug_console_help("[help]"), None);
        for expected in [
            "inspect operand  Show linked values",
            "[]                         No Concept",
            "(expression)               Make one complete surrounding operand",
            "[A]*[B]                    Merge unordered Concept members",
            "[A]/[B]",
            "x2[A]x3[B]                 Signed integer coefficients",
            "{name} ~= expression     Capture one experience",
            "subject @ expression       Complete a Concept",
            "{source} @ expression    Complete one retained Percept source",
            "{*} @ expression         Complete the global Percept's Concepts",
            "{a}{b} @ expression   Complete several retained sources together",
            "&operand                   Return the shared answer shape",
            "{target} @+= {evidence} Add matching evidence",
            "{target} @-= {evidence} Subtract matching evidence",
            "${*}                     Inspect all live ordinary Concepts",
            "Repeating an equal Concept adds default relevance",
            "^{choice}",
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
        assert_eq!(debug_console_inspection_operand("inspect {choice}"), Some("{choice}"));
        assert_eq!(debug_console_inspection_operand("inspect\t{choice}"), Some("{choice}"));
        assert_eq!(debug_console_inspection_operand("inspect"), Some(""));
        assert_eq!(debug_console_inspection_operand("inspector {choice}"), None);
        assert_eq!(debug_console_inspection_operand("[inspect]"), None);
    }

    #[test]
    fn debug_console_inspection_shows_values_rows_ties_and_complete_sources() {
        let mut pangine = Pangine::new();
        for script in [
            "{candidates} ~= [A]",
            "{candidates} ~= [B]",
            "{helpful} ~= [A]",
            "{failed} ~= [A]",
            "{failed} ~= [A]",
            "{candidates} @ {choice}",
            "{helpful} @ {helpful-choice}",
            "{choice} @+= {helpful-choice}",
            "{failed} @ {failed-choice}",
            "{choice} @-= {failed-choice}",
        ] {
            assert!(pangine.reference_concept(script).unwrap().is_some(), "expected a Concept from {script}");
        }

        assert_eq!(
            pangine.debug_answer_inspection_lines("{choice}"),
            Ok(vec![
                "    +0, 1 row: [A]".to_owned(),
                "      +1 from {candidates}: [A]".to_owned(),
                "      -2 from {failed}: [A]".to_owned(),
                "      +1 from {helpful}: [A]".to_owned(),
                "  * +1, 1 row: [B]".to_owned(),
                "      +1 from {candidates}: [B]".to_owned(),
            ])
        );
        assert_eq!(pangine.debug_answer_inspection_lines("{missing}"), Err("operand is not part of one linked Answer".to_owned()));
    }
}

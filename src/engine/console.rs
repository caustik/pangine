//! The interactive console, its help text, and diagnostic output lines.

use super::{format::format_x_coefficient, CompletionGrade, ConceptId, ConceptKind, Pangine};
use crate::Relevance;
use std::io::{self, Write};

const DEBUG_CONSOLE_HELP: &str = "\
Commands:
  help, h          Show this help
  inspect operand  Show linked values, their probabilities, and their sources
  seed n           Restart the generator that ^~ draws from at seed n
  quit, q          Exit the command-line console

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
  x2[A]x3[B]                 Signed evidence counts

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
  subject @~ expression      Graded: compose parts, generalize from cases
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
  remain references. Repeating an equal Concept adds one to that member's
  evidence count. Questions derive recursive matches without multiplying one
  experience by match routes.

Scripts:
  expression; expression    Multiple statements
  // line comment            C++-style comment
  /* block comment */        C-style comment

Relevance and choice:
  Coefficients count evidence. A value's probability is its share of the
  positive evidence among the alternatives, so x2[tea]x3[coffee] reads as tea
  2/5 and coffee 3/5. Evidence at or below zero has probability 0. A row joined
  from separate experiences weighs the product of their counts. A graded @~
  answer interpolates its probabilities from exact rows toward composed ones,
  then toward single cases that differ from the question in a few names.
  Evidence that @+= or @-= imports from a graded answer keeps its grade, and a
  graded answer cannot itself be adjusted.

  ^operand chooses the most probable current result. For output Percepts from
  one question, it removes incompatible answers and refreshes every linked
  output. Several output Percepts in one operand are chosen together.

  ^~operand draws a result instead, with probability equal to its share, and
  collapses the answer the same way. Each engine draws from its own seeded
  generator, so a new console starting from seed 0 repeats its draws exactly.

  {choice} = x2[tea]x3[coffee]
  ^{choice}             returns [coffee]
  ^~{choice}            returns [tea] with probability 2/5
  ^({animal}->{food}) chooses one complete animal-food pair

  Exact ties use the earliest canonical Concept spelling. If no entry has
  positive evidence, ^ and ^~ return []. Zero counts disappear when their
  Concept is built and are not decision candidates.
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
            let probability = possibility.probability();
            lines.push(format!("  {marker} {:+}, p={probability}, {rows} {row_label}: {value}", possibility.strength().count()));

            let mut support = possibility
                .support()
                .iter()
                .map(|support| {
                    let sources = support
                        .sources()
                        .iter()
                        .map(|source| {
                            let subject = self.format_concept(source.subject(), false);
                            (subject, self.format_concept(source.concept(), false), self.format_debug_console_member(source.relevance(), source.concept()))
                        })
                        .collect::<Vec<_>>();
                    (support.grade(), sources, support.weight().count())
                })
                .collect::<Vec<_>>();
            support.sort();

            for (grade, sources, weight) in support {
                let label = match grade {
                    CompletionGrade::Exact => String::new(),
                    CompletionGrade::Composed => " composed".to_owned(),
                    CompletionGrade::Generalized { distance } => format!(" at distance {distance}"),
                };
                if let [(subject, source, _)] = sources.as_slice() {
                    lines.push(format!("      {weight:+}{label} from {subject}: {source}"));
                    continue;
                }
                lines.push(format!("      {weight:+}{label} from {} sources:", sources.len()));
                for (subject, _, member) in sources {
                    lines.push(format!("        {subject}: {member}"));
                }
            }
        }
        Ok(lines)
    }

    fn debug_console_seed(&mut self, operand: &str) -> Result<(), String> {
        let seed = operand.trim_end().parse().map_err(|_| format!("seed expects one whole number from 0 to {}", u64::MAX))?;
        self.set_sample_seed(seed);
        Ok(())
    }

    /// Runs one console command: `help` or `h`, `inspect operand`, or
    /// `seed n`.
    ///
    /// Returns the lines the command prints, or its message when it fails.
    /// Returns none when `line` is not a console command, so the caller can run
    /// it as Pangine syntax. The interactive console and the pangine.com
    /// workbench share these commands.
    pub fn debug_console_command(&mut self, line: &str) -> Option<Result<Vec<String>, String>> {
        if let Some(help) = debug_console_help(line) {
            return Some(Ok(help.lines().map(str::to_owned).collect()));
        }
        if let Some(operand) = debug_console_command_operand(line, "inspect") {
            return Some(self.debug_answer_inspection_lines(operand));
        }
        let operand = debug_console_command_operand(line, "seed")?;
        Some(self.debug_console_seed(operand).map(|()| Vec::new()))
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

            let output = match self.debug_console_command(script) {
                Some(output) => output,
                None => self.reference_concept(script).map(|concept| self.debug_console_lines(concept.as_ref())).map_err(|error| error.to_string()),
            };
            match output {
                Ok(lines) => {
                    for line in lines {
                        println!("{line}");
                    }
                }
                Err(error) => println!("  {error}"),
            }
        }

        Ok(())
    }

    fn format_debug_console_line(&self, relevance: Relevance, concept: &ConceptId) -> String {
        format!("  {}", self.format_debug_console_member(relevance, concept))
    }

    fn format_debug_console_member(&self, relevance: Relevance, concept: &ConceptId) -> String {
        let mut out = String::new();
        let count = relevance.count();
        let add_separator = count != 1 && count != -1;
        let wrap_concept = count != 1 && matches!(concept.0.kind, ConceptKind::Unordered | ConceptKind::Ordered { .. });

        if count == -1 {
            out.push('!');
        }

        if count != 1 && count != -1 {
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

// A console command is its name alone, or its name, whitespace, and an operand.
fn debug_console_command_operand<'a>(command: &'a str, name: &str) -> Option<&'a str> {
    let operand = command.strip_prefix(name)?;
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
            "x2[A]x3[B]                 Signed evidence counts",
            "{name} ~= expression     Capture one experience",
            "subject @ expression       Complete a Concept",
            "{source} @ expression    Complete one retained Percept source",
            "{*} @ expression         Complete the global Percept's Concepts",
            "{a}{b} @ expression   Complete several retained sources together",
            "subject @~ expression      Graded: compose parts, generalize from cases",
            "seed n           Restart the generator that ^~ draws from",
            "^~{choice}            returns [tea] with probability 2/5",
            "imports from a graded answer keeps its grade",
            "&operand                   Return the shared answer shape",
            "{target} @+= {evidence} Add matching evidence",
            "{target} @-= {evidence} Subtract matching evidence",
            "${*}                     Inspect all live ordinary Concepts",
            "Repeating an equal Concept adds one to that member's",
            "x2[tea]x3[coffee] reads as tea",
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
        assert_eq!(debug_console_command_operand("inspect {choice}", "inspect"), Some("{choice}"));
        assert_eq!(debug_console_command_operand("inspect\t{choice}", "inspect"), Some("{choice}"));
        assert_eq!(debug_console_command_operand("inspect", "inspect"), Some(""));
        assert_eq!(debug_console_command_operand("inspector {choice}", "inspect"), None);
        assert_eq!(debug_console_command_operand("[inspect]", "inspect"), None);
        assert_eq!(debug_console_command_operand("seed 7", "seed"), Some("7"));
        assert_eq!(debug_console_command_operand("seeds 7", "seed"), None);
    }

    #[test]
    fn console_commands_are_shared_and_other_lines_are_left_to_the_caller() {
        let mut pangine = Pangine::new();
        let help = pangine.debug_console_command("help").expect("help is a console command").expect("help prints");
        assert_eq!(help.first().map(String::as_str), Some("Commands:"));
        assert_eq!(help.join("\n") + "\n", DEBUG_CONSOLE_HELP);

        for script in ["{world} ~= [morning]->[birds]", "{world} ~= [morning]->[birds]", "{world} ~= [morning]->[traffic]", "{world} @ [morning]->{answer}"] {
            assert_eq!(pangine.debug_console_command(script), None, "{script} is Pangine syntax");
            pangine.reference_concept(script).unwrap();
        }
        assert_eq!(pangine.debug_console_command("inspect {answer}"), Some(pangine.debug_answer_inspection_lines("{answer}")));
        assert_eq!(pangine.debug_console_command("inspect {missing}"), Some(Err("operand is not part of one linked Answer".to_owned())));
        assert_eq!(pangine.debug_console_command("seed 7"), Some(Ok(Vec::new())));
        assert!(matches!(pangine.debug_console_command("seed seven"), Some(Err(_))));
        for line in ["quit", "q", "seeds 7", "inspector", "[help]"] {
            assert_eq!(pangine.debug_console_command(line), None, "{line} is not a shared console command");
        }
    }

    #[test]
    fn the_seed_command_restarts_the_generator_that_sampling_draws_from() {
        let draws = |seed: Option<&str>| {
            let mut pangine = Pangine::new();
            if let Some(seed) = seed {
                pangine.debug_console_seed(seed).expect("a valid seed");
            }
            pangine.reference_concept("{choice} = x2[tea]x3[coffee]").unwrap();
            (0..24)
                .map(|_| {
                    let value = pangine.reference_concept("^~{choice}").unwrap().expect("a drawn value");
                    pangine.format_concept(&value, false)
                })
                .collect::<Vec<_>>()
        };

        assert_eq!(draws(Some("0")), draws(None), "a new engine starts from seed 0");
        assert_eq!(draws(Some("7")), draws(Some("7")));
        assert_ne!(draws(Some("7")), draws(None));

        let mut pangine = Pangine::new();
        let expected = Err(format!("seed expects one whole number from 0 to {}", u64::MAX));
        for operand in ["", "-1", "seven", "18446744073709551616"] {
            assert_eq!(pangine.debug_console_seed(operand), expected, "operand {operand:?}");
        }
        assert_eq!(pangine.debug_console_seed("18446744073709551615"), Ok(()));
        assert_eq!(pangine.debug_console_seed("7 "), Ok(()));
    }

    #[test]
    fn debug_console_inspection_reads_repeated_experience_as_probabilities() {
        let mut pangine = Pangine::new();
        for script in ["{world} ~= [morning]->[birds]", "{world} ~= [morning]->[birds]", "{world} ~= [morning]->[traffic]", "{world} @ [morning]->{answer}"] {
            assert!(pangine.reference_concept(script).unwrap().is_some(), "expected a Concept from {script}");
        }

        assert_eq!(
            pangine.debug_answer_inspection_lines("{answer}"),
            Ok(vec![
                "  * +2, p=2/3, 1 row: [birds]".to_owned(),
                "      +2 from {world}: [morning]->[birds]".to_owned(),
                "    +1, p=1/3, 1 row: [traffic]".to_owned(),
                "      +1 from {world}: [morning]->[traffic]".to_owned(),
            ])
        );
    }

    #[test]
    fn debug_console_inspection_lists_each_source_of_a_joined_row() {
        let mut pangine = Pangine::new();
        for script in [
            "{knowledge} ~= [Socrates]->[is-a]->[human]",
            "{knowledge} ~= [Socrates]->[is-a]->[human]",
            "{knowledge} ~= [human]->[is-a]->[mortal]",
            "{knowledge} @ ([Socrates]->[is-a]->{kind})({kind}->[is-a]->{conclusion})",
        ] {
            assert!(pangine.reference_concept(script).unwrap().is_some(), "expected a Concept from {script}");
        }

        assert_eq!(
            pangine.debug_answer_inspection_lines("{conclusion}"),
            Ok(vec![
                "  * +2, p=1, 1 row: [mortal]".to_owned(),
                "      +2 from 2 sources:".to_owned(),
                "        {knowledge}: x2([Socrates]->[is-a]->[human])".to_owned(),
                "        {knowledge}: [human]->[is-a]->[mortal]".to_owned(),
            ])
        );
    }

    #[test]
    fn debug_console_inspection_labels_rows_composed_from_parts_seen_separately() {
        let mut pangine = Pangine::new();
        for script in ["{closet} ~= ([top]->[red])([bottom]->[jeans])", "{closet} ~= [top]->[green]", "{closet} @~ ([top]->{shirt})([bottom]->{pants})"] {
            assert!(pangine.reference_concept(script).unwrap().is_some(), "expected a Concept from {script}");
        }

        assert_eq!(
            pangine.debug_answer_inspection_lines("{shirt}->{pants}"),
            Ok(vec![
                "  * +1, p=3/4, 1 row: [red]->[jeans]".to_owned(),
                "      +1 from {closet}: ([bottom]->[jeans])([top]->[red])".to_owned(),
                "    +1, p=1/4, 1 row: [green]->[jeans]".to_owned(),
                "      +1 composed from 2 sources:".to_owned(),
                "        {closet}: ([bottom]->[jeans])([top]->[red])".to_owned(),
                "        {closet}: [top]->[green]".to_owned(),
            ])
        );
    }

    #[test]
    fn debug_console_inspection_gives_the_distance_of_a_generalized_case() {
        let mut pangine = Pangine::new();
        for script in ["{games} ~= [x]->[_]->[o]->[c2]", "{games} ~= [x]->[_]->[_]->[c3]", "{games} @~ [x]->[_]->[o]->{move}"] {
            assert!(pangine.reference_concept(script).unwrap().is_some(), "expected a Concept from {script}");
        }

        assert_eq!(
            pangine.debug_answer_inspection_lines("{move}"),
            Ok(vec![
                "  * +1, p=3/4, 1 row: [c2]".to_owned(),
                "      +1 from {games}: [x]->[_]->[o]->[c2]".to_owned(),
                "    +1, p=1/4, 1 row: [c3]".to_owned(),
                "      +1 at distance 1 from {games}: [x]->[_]->[_]->[c3]".to_owned(),
            ])
        );
    }

    #[test]
    fn debug_console_inspection_labels_imported_evidence_by_its_grade() {
        let mut pangine = Pangine::new();
        for script in [
            "{options} ~= [hall]->[north]->[left]",
            "{options} ~= [hall]->[north]->[right]",
            "{trips} ~= [lobby]->[north]->[left]",
            "{options} @ [hall]->[north]->{way}",
            "{trips} @~ [hall]->[north]->{trip-way}",
            "{way} @+= {trip-way}",
        ] {
            assert!(pangine.reference_concept(script).unwrap().is_some(), "expected a Concept from {script}");
        }

        assert_eq!(
            pangine.debug_answer_inspection_lines("{way}"),
            Ok(vec![
                "  * +2, p=7/12, 1 row: [left]".to_owned(),
                "      +1 from {options}: [hall]->[north]->[left]".to_owned(),
                "      +1 at distance 1 from {trips}: [lobby]->[north]->[left]".to_owned(),
                "    +1, p=5/12, 1 row: [right]".to_owned(),
                "      +1 from {options}: [hall]->[north]->[right]".to_owned(),
            ])
        );
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
                "  * +1, p=1, 1 row: [B]".to_owned(),
                "      +1 from {candidates}: [B]".to_owned(),
                "    +0, p=0, 1 row: [A]".to_owned(),
                "      +1 from {candidates}: [A]".to_owned(),
                "      -2 from {failed}: [A]".to_owned(),
                "      +1 from {helpful}: [A]".to_owned(),
            ])
        );
        assert_eq!(pangine.debug_answer_inspection_lines("{missing}"), Err("operand is not part of one linked Answer".to_owned()));
    }
}

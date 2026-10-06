//! Dividing remembered experience among partitions must not change any result
//! a program reads.

use pangine::Pangine;
use std::path::Path;

const PARTITIONS: [usize; 3] = [2, 3, 7];

#[test]
fn example_programs_print_the_same_results_with_divided_memory() {
    let examples = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples");
    let mut programs = std::fs::read_dir(&examples)
        .expect("the examples directory")
        .map(|entry| entry.expect("an example").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "pae"))
        .collect::<Vec<_>>();
    programs.sort();
    assert!(programs.len() >= 4, "the example programs: {programs:?}");

    for program in programs {
        let script = std::fs::read_to_string(&program).expect("a readable example");
        let expected = transcript(&script, 1);
        for partitions in PARTITIONS {
            assert_eq!(transcript(&script, partitions), expected, "{} with {partitions} partitions", program.display());
        }
    }
}

#[test]
fn a_learning_loop_reads_the_same_with_divided_memory() {
    // The loop has the tic-tac-toe demo's shape. A position gains starting
    // evidence for its moves, every move asks exact and graded questions,
    // adjusts by them, and chooses or draws, and each outcome is remembered.
    let mut engines = std::iter::once(1)
        .chain(PARTITIONS)
        .map(|partitions| {
            let mut pangine = Pangine::new();
            assert!(pangine.set_partitions(partitions));
            pangine
        })
        .collect::<Vec<_>>();
    let mut state = 0x5EED;
    let positions = (0..10).map(|_| (0..9).map(|_| ["[me]", "[you]", "[_]"][next(&mut state) % 3]).collect::<Vec<_>>().join("->")).collect::<Vec<_>>();

    let mut choices = 0;
    for round in 0..80 {
        let position = &positions[next(&mut state) % positions.len()];
        if step(&mut engines, &format!("{{prior}} @ {position}->{{move}}")) == "[]" {
            for cell in 1..=9 {
                if next(&mut state).is_multiple_of(2) {
                    step(&mut engines, &format!("{{prior}} ~= {position}->[c{cell}]"));
                }
            }
            step(&mut engines, &format!("{{prior}} @ {position}->{{move}}"));
        }
        if step(&mut engines, &format!("{{won}} @~ {position}->{{won-move}}")) != "[]" {
            step(&mut engines, "{move} @+= {won-move}");
        }
        if step(&mut engines, &format!("{{lost}} @ {position}->{{lost-move}}")) != "[]" {
            step(&mut engines, "{move} @-= {lost-move}");
        }
        inspect(&mut engines, "{move}");

        let chosen = step(&mut engines, if round % 2 == 0 { "^{move}" } else { "^~{move}" });
        if chosen != "[]" {
            choices += 1;
            let outcome = if next(&mut state).is_multiple_of(3) { "lost" } else { "won" };
            step(&mut engines, &format!("{{{outcome}}} ~= {position}->{chosen}"));
        }
    }
    assert!(choices > 40, "the loop must keep choosing moves: {choices}");
    inspect(&mut engines, "{move}");
}

// Runs a script and returns every statement with its result.
fn transcript(script: &str, partitions: usize) -> String {
    let mut pangine = Pangine::new();
    assert!(pangine.set_partitions(partitions));
    let mut details = Vec::new();
    pangine.parse_script_text_with_details(script, &mut details).expect("the example runs");
    String::from_utf8(details).expect("UTF-8 details")
}

// Runs one statement in every engine and returns the result they agree on.
fn step(engines: &mut [Pangine], statement: &str) -> String {
    let results = engines
        .iter_mut()
        .map(|pangine| {
            let value = pangine.reference_concept(statement).unwrap_or_else(|error| panic!("failed to run {statement:?}: {error}"));
            value.map_or_else(|| "[]".to_owned(), |value| pangine.format_concept(&value, false))
        })
        .collect::<Vec<_>>();
    assert!(results.iter().all(|result| *result == results[0]), "{statement}: {results:#?}");
    results[0].clone()
}

// Inspects one operand in every engine and checks that they agree.
fn inspect(engines: &mut [Pangine], operand: &str) {
    let outputs = engines.iter_mut().map(|pangine| pangine.debug_console_command(&format!("inspect {operand}"))).collect::<Vec<_>>();
    assert!(outputs.iter().all(|output| *output == outputs[0]), "inspect {operand}: {outputs:#?}");
}

// A small seeded generator (splitmix64), so the loop repeats exactly.
fn next(state: &mut u64) -> usize {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut value = *state;
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    (value ^ (value >> 31)) as usize
}

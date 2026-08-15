//! Warning check for applying recorded outcomes to a linked decision.
//!
//! The program gives each candidate and episode a stable identity. Helpful and
//! failed episodes are selected with ordinary questions, then their complete
//! decision outputs adjust the still-linked candidate answer. The outcome
//! meanings and additive effects are explicit program choices, not universal
//! Pangine semantics.

use pangine::{ConceptId, Pangine, Relevance};
use std::collections::BTreeMap;

const CANDIDATE_QUESTION: &str = "
    (['candidate']->[environment]->$['environment-input'])
    (['candidate']->[symptom]->$['symptom-input'])
    (['candidate']->[decision]->['decision'])";

#[test]
#[ignore = "warning: helpful-minus-failed linked support is one explicit outcome policy"]
fn linked_outcomes_adjust_complete_troubleshooting_choices_without_hiding_sources() {
    let mut pangine = Pangine::new();
    remember_candidate(&mut pangine, "candidate-clean", "clean-build", "cargo");
    remember_candidate(&mut pangine, "candidate-inspect", "inspect-symbols", "dumpbin");
    remember_candidate(&mut pangine, "candidate-reconfigure", "reconfigure", "cmake");

    remember_episode(&mut pangine, "episode-clean-failed-1", "clean-build", "cargo", "failed");
    remember_episode(&mut pangine, "episode-clean-failed-2", "clean-build", "cargo", "failed");
    remember_episode(&mut pangine, "episode-inspect-helpful", "inspect-symbols", "dumpbin", "helpful");

    must_ref(&mut pangine, "['environment-input'] = [windows]");
    must_ref(&mut pangine, "['symptom-input'] = [link-error]");
    must_ref(&mut pangine, &format!("['candidates'] @ {CANDIDATE_QUESTION}"));

    assert_eq!(must_ref(&mut pangine, "&['decision']"), must_ref(&mut pangine, CANDIDATE_QUESTION));
    assert_eq!(
        must_ref(&mut pangine, "$['decision']"),
        must_ref(&mut pangine, "([clean-build]->[cargo])([inspect-symbols]->[dumpbin])([reconfigure]->[cmake])")
    );

    ask_outcome_decisions(&mut pangine, "helpful", "helpful");
    ask_outcome_decisions(&mut pangine, "failed", "failed");

    let decision = pangine.reference_percept("decision");
    let helpful_decision = pangine.reference_percept("helpful-decision");
    let failed_decision = pangine.reference_percept("failed-decision");
    let candidates = pangine.answer_view(&decision).expect("candidate answer");
    let helpful = pangine.answer_view(&helpful_decision).expect("helpful answer");
    let failed = pangine.answer_view(&failed_decision).expect("failed answer");
    let adjusted = candidates.adjust(&mut pangine, &helpful, Relevance::DEFAULT).expect("helpful adjustment").into_view();
    let adjusted = adjusted.adjust(&mut pangine, &failed, Relevance::new(-1)).expect("failed adjustment").into_view();
    adjusted.answer().publish(&mut pangine).expect("current candidate revision");

    assert_eq!(must_ref(&mut pangine, "&['decision']"), must_ref(&mut pangine, CANDIDATE_QUESTION));
    assert_eq!(
        must_ref(&mut pangine, "$['decision']"),
        must_ref(&mut pangine, "x2([inspect-symbols]->[dumpbin])([reconfigure]->[cmake])!([clean-build]->[cargo])")
    );
    assert_eq!(must_ref(&mut pangine, "$['candidate']"), must_ref(&mut pangine, "x2[candidate-inspect][candidate-reconfigure]![candidate-clean]"));

    let answer = pangine.answer_snapshot(&decision).expect("published decision answer");
    let source_inventory = answer
        .result()
        .completions()
        .iter()
        .map(|completion| {
            let choice = pangine.format_concept(completion.binding(&decision).expect("decision binding"), false);
            let sources = completion
                .evidence()
                .iter()
                .map(|evidence| {
                    let source = pangine.format_concept(evidence.source_concept(), false);
                    (source, (evidence.source_relevance().weight(), evidence.source_contribution().weight()))
                })
                .collect::<BTreeMap<_, _>>();
            (choice, sources)
        })
        .collect::<BTreeMap<_, _>>();
    assert_sources(&source_inventory, "{[clean-build]->[cargo]}", &["candidate-clean", "episode-clean-failed-1", "episode-clean-failed-2"]);
    assert_sources(&source_inventory, "{[inspect-symbols]->[dumpbin]}", &["candidate-inspect", "episode-inspect-helpful"]);
    assert_sources(&source_inventory, "{[reconfigure]->[cmake]}", &["candidate-reconfigure"]);
    assert!(source_inventory["{[clean-build]->[cargo]}"]
        .iter()
        .filter(|(source, _)| source.contains("episode-clean-failed"))
        .all(|(_, relevance)| *relevance == (1, -1)));

    must_ref(&mut pangine, "['selected-decision'] = ^['decision']");
    assert_eq!(must_ref(&mut pangine, "$['selected-decision']"), must_ref(&mut pangine, "[inspect-symbols]->[dumpbin]"));

    must_ref(
        &mut pangine,
        "['episodes'] @
           (['supporting-episode']->[environment]->$['environment-input'])
           (['supporting-episode']->[symptom]->$['symptom-input'])
           (['supporting-episode']->[decision]->($['selected-decision']))
           (['supporting-episode']->[outcome]->['supporting-outcome'])",
    );
    assert_eq!(must_ref(&mut pangine, "$['supporting-episode']"), must_ref(&mut pangine, "[episode-inspect-helpful]"));
    assert_eq!(must_ref(&mut pangine, "$['supporting-outcome']"), must_ref(&mut pangine, "[helpful]"));

    must_ref(&mut pangine, "['episodes'] @ [episode-clean-failed-1]->[outcome]->['recorded-failure']");
    assert_eq!(must_ref(&mut pangine, "$['recorded-failure']"), must_ref(&mut pangine, "[failed]"));
}

fn remember_candidate(pangine: &mut Pangine, candidate: &str, action: &str, tool: &str) {
    must_ref(
        pangine,
        &format!(
            "['candidates'] ~= ([{candidate}]->[environment]->[windows])
                               ([{candidate}]->[symptom]->[link-error])
                               ([{candidate}]->[decision]->([{action}]->[{tool}]))"
        ),
    );
}

fn remember_episode(pangine: &mut Pangine, episode: &str, action: &str, tool: &str, outcome: &str) {
    must_ref(
        pangine,
        &format!(
            "['episodes'] ~= ([{episode}]->[environment]->[windows])
                             ([{episode}]->[symptom]->[link-error])
                             ([{episode}]->[decision]->([{action}]->[{tool}]))
                             ([{episode}]->[outcome]->[{outcome}])"
        ),
    );
}

fn ask_outcome_decisions(pangine: &mut Pangine, role: &str, outcome: &str) {
    must_ref(
        pangine,
        &format!(
            "['episodes'] @
               (['{role}-episode']->[environment]->$['environment-input'])
               (['{role}-episode']->[symptom]->$['symptom-input'])
               (['{role}-episode']->[decision]->['{role}-decision'])
               (['{role}-episode']->[outcome]->[{outcome}])"
        ),
    );
}

fn assert_sources(inventory: &BTreeMap<String, BTreeMap<String, (i64, i64)>>, decision: &str, expected_sources: &[&str]) {
    let sources = &inventory[decision];
    for expected in expected_sources {
        assert!(sources.keys().any(|source| source.contains(expected)), "missing {expected} from {decision}: {sources:?}");
    }
    assert_eq!(sources.len(), expected_sources.len());
}

fn must_ref(pangine: &mut Pangine, input: &str) -> ConceptId {
    pangine
        .reference_concept(input)
        .unwrap_or_else(|error| panic!("failed to parse {input:?}: {error}"))
        .unwrap_or_else(|| panic!("expected non-null Concept for {input:?}"))
}

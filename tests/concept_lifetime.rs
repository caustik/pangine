use pangine::{Pangine, ParseError};

// Historical anchors:
// 3.x/pangine/include/pangine/pae_pangine.h:25-27
// 3.x/pangine/include/pangine/pae_shared_concept.h:26-32
// 3.x/pangine/src/test/common/test_pangine.cpp:705-710
#[test]
fn ordinary_concepts_live_as_long_as_owning_handles() {
    let mut pangine = Pangine::new();
    let concept = pangine.reference_concept("[A][B]").unwrap().unwrap();

    assert_eq!(pangine.concept_count(), 3);

    let second_handle = concept.clone();
    drop(concept);
    assert_eq!(pangine.concept_count(), 3);

    drop(second_handle);
    assert_eq!(pangine.concept_count(), 0);
}

// Historical anchor:
// 3.x/pangine/src/libpangine/common/pae_pangine.cpp:344-388
#[test]
fn failed_pure_parses_release_transient_concepts() {
    let mut pangine = Pangine::new();

    assert!(matches!(pangine.reference_concept("([A][B]"), Err(ParseError::InvalidSyntax)));
    assert_eq!(pangine.concept_count(), 0);
}

#[test]
fn percept_values_retain_concepts_until_cleared() {
    let mut pangine = Pangine::new();
    let percept = pangine.reference_percept("memory");
    let value = pangine.reference_concept("[A][B]").unwrap().unwrap();

    assert!(pangine.set_percept_value(&percept, Some(value.clone())));
    drop(value);
    assert_eq!(pangine.concept_count(), 3);

    assert!(pangine.set_percept_value(&percept, None));
    assert_eq!(pangine.concept_count(), 0);
}

// Percept assignment is eager in the 3.x design as well:
// 3.x/pangine/src/libpangine/common/pae_pangine.cpp:186-195
#[test]
fn parse_failures_do_not_roll_back_prior_percept_mutations() {
    let mut pangine = Pangine::new();
    let percept = pangine.reference_percept("state");

    drop(pangine.reference_concept("['state'] = [before]").unwrap());
    assert!(matches!(pangine.reference_concept("['state'] = [after]; ([broken]"), Err(ParseError::InvalidSyntax)));

    let after = pangine.reference_concept("[after]").unwrap().unwrap();
    assert_eq!(pangine.get_value(&percept), Some(after.clone()));
    assert_eq!(pangine.concept_count(), 1);

    drop(after);
    assert!(pangine.set_percept_value(&percept, None));
    assert_eq!(pangine.concept_count(), 0);
}

#[test]
fn owning_handles_cannot_cross_engine_state_boundaries() {
    let mut first = Pangine::new();
    let mut second = Pangine::new();
    let foreign = first.reference_concept("[A]").unwrap().unwrap();
    let local_percept = second.reference_percept("memory");

    assert!(matches!(second.reference_concept_with_params("[%]", std::slice::from_ref(&foreign)), Err(ParseError::InvalidSyntax)));
    assert!(!second.set_percept_value(&local_percept, Some(foreign.clone())));
    assert_eq!(second.perform_addition(&local_percept, Some(&foreign)), None);
    assert_eq!(second.concept_count(), 0);
}

#[test]
fn global_percept_is_a_read_only_computed_view() {
    let mut pangine = Pangine::new();
    let global = pangine.global_percept();

    assert_eq!(pangine.reference_concept("['*']").unwrap(), Some(global.clone()));
    assert_eq!(pangine.format_concept(&global, false), "['*']");
    assert_eq!(pangine.get_value(&global), None);
    assert_eq!(pangine.reference_concept("$['*']").unwrap(), None);
    assert!(!pangine.set_percept_value(&global, None));
    assert!(matches!(pangine.reference_concept("['*'] = [A]"), Err(ParseError::InvalidSyntax)));
    assert_eq!(pangine.concept_count(), 0);
}

#[test]
fn global_percept_snapshots_every_live_ordinary_concept() {
    let mut pangine = Pangine::new();
    let global = pangine.global_percept();
    let pair = pangine.reference_concept("[A][B]").unwrap().unwrap();
    let snapshot = pangine.reference_concept("$['*']").unwrap().unwrap();

    assert_eq!(pangine.get_relevance_map(&snapshot).len(), 3);
    assert_eq!(pangine.format_concept(&snapshot, false), "[A][B]([A][B])");
    assert_eq!(pangine.format_concept(&global, true), "[A][B]([A][B])");
    assert_eq!(pangine.concept_count(), 3);

    drop(pair);
    assert_eq!(pangine.concept_count(), 3);

    drop(snapshot);
    assert_eq!(pangine.concept_count(), 0);
}

#[test]
fn global_snapshot_expands_shared_concepts_instead_of_unlabeled_references() {
    let mut pangine = Pangine::new();
    pangine.reference_concept("['memory'] ~= {[C]->[A]}*{[B]->[D]}").unwrap().unwrap();
    let snapshot = pangine.reference_concept("$['*']").unwrap().unwrap();
    let lines = pangine.debug_console_lines(Some(&snapshot), false);

    assert!(lines.iter().all(|line| !line.contains("[#")));
    assert_eq!(lines.last().map(String::as_str), Some("  <?[]:[A], ?[]:[B], ?[]:[C], ?[]:[D], ?[]:{[B]->[D]}, ?[]:{[C]->[A]}, ?[]:{[B]->[D]}{[C]->[A]}>"));
}

#[test]
fn evaluated_formatting_stops_a_percept_cycle_at_its_named_reference() {
    let mut pangine = Pangine::new();
    let cycle = pangine.reference_concept("['cycle'] = ['cycle']").unwrap().unwrap();

    assert_eq!(pangine.format_concept(&cycle, true), "['cycle']");
}

#[test]
fn global_percept_excludes_named_percept_roots() {
    let mut pangine = Pangine::new();
    let global = pangine.global_percept();
    let memory = pangine.reference_percept("memory");

    assert_eq!(pangine.get_value(&global), None);
    assert_eq!(pangine.concept_count(), 0);
    drop(memory);
}

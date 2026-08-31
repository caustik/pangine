//! Warning check for replaceable current records with represented membership.
//!
//! Each record lives in one Percept, so assignment replaces that record and a
//! missing value clears it. A separate current Percept represents membership as
//! ordinary relationships to those record Percepts. One question recovers the
//! member identities, and a second questions the records through the existing
//! multi-Percept API. Pangine does not enforce logical keys, persist the state,
//! or roll back failed application operations.
//! Category, key, value, and version are ordinary parser-safe names. This test
//! does not establish arbitrary scalar or missing-value representation.

use pangine::{CompletionResult, ConceptId, Pangine};
use std::collections::BTreeSet;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct CurrentRecord {
    record: String,
    category: String,
    key: String,
    value: String,
    version: String,
}

#[test]
#[ignore = "warning: represented membership discovers current records, but logical keys, arbitrary values, persistence, and transaction rollback remain unresolved"]
fn represented_members_support_complete_query_filter_replace_and_clear() {
    let mut pangine = Pangine::new();
    let collection = pangine.reference_percept("current-record-collection");
    let member_a = pangine.reference_percept("collection-member-a");
    let member_b = pangine.reference_percept("collection-member-b");

    let initial_a = record(&mut pangine, "record-a", "category-shared", "key-a", "value-old", "version-one");
    let initial_b = record(&mut pangine, "record-b", "category-shared", "key-b", "value-stable", "version-one");
    let initial_membership = collection_membership(&mut pangine, "collection-member", &[member_a.clone(), member_b.clone()]);
    pangine
        .set_percept_values(&[(collection.clone(), Some(initial_membership)), (member_a.clone(), Some(initial_a)), (member_b.clone(), Some(initial_b))])
        .expect("valid initial collection");

    assert_eq!(discover_members(&mut pangine, &collection, "collection-member"), BTreeSet::from([member_a.clone(), member_b.clone()]));
    assert_eq!(query_records(&mut pangine, &collection), initial_records());
    assert_eq!(
        query_values_in_category(&mut pangine, &collection, "category-shared"),
        BTreeSet::from([
            ("record-a".to_owned(), "key-a".to_owned(), "value-old".to_owned()),
            ("record-b".to_owned(), "key-b".to_owned(), "value-stable".to_owned()),
        ])
    );
    assert!(query_values_in_category(&mut pangine, &collection, "category-other").is_empty());

    let membership_before_replacement = pangine.get_value(&collection).expect("represented membership");
    let replacement_a = record(&mut pangine, "record-a", "category-shared", "key-a", "value-new", "version-two");
    pangine.set_percept_values(&[(member_a.clone(), Some(replacement_a))]).expect("valid replacement");

    assert_eq!(pangine.get_value(&collection), Some(membership_before_replacement));
    assert_eq!(
        query_records(&mut pangine, &collection),
        BTreeSet::from([
            CurrentRecord {
                record: "record-a".to_owned(),
                category: "category-shared".to_owned(),
                key: "key-a".to_owned(),
                value: "value-new".to_owned(),
                version: "version-two".to_owned(),
            },
            CurrentRecord {
                record: "record-b".to_owned(),
                category: "category-shared".to_owned(),
                key: "key-b".to_owned(),
                value: "value-stable".to_owned(),
                version: "version-one".to_owned(),
            },
        ])
    );

    let remaining_membership = collection_membership(&mut pangine, "collection-member", std::slice::from_ref(&member_a));
    pangine
        .set_percept_values(&[(collection.clone(), Some(remaining_membership)), (member_b, None)])
        .expect("valid grouped membership removal and record clear");

    assert_eq!(discover_members(&mut pangine, &collection, "collection-member"), BTreeSet::from([member_a.clone()]));
    assert_eq!(
        query_records(&mut pangine, &collection),
        BTreeSet::from([CurrentRecord {
            record: "record-a".to_owned(),
            category: "category-shared".to_owned(),
            key: "key-a".to_owned(),
            value: "value-new".to_owned(),
            version: "version-two".to_owned(),
        }])
    );
    assert_eq!(
        query_values_in_category(&mut pangine, &collection, "category-shared"),
        BTreeSet::from([("record-a".to_owned(), "key-a".to_owned(), "value-new".to_owned())])
    );

    pangine.set_percept_values(&[(collection.clone(), None), (member_a, None)]).expect("valid grouped collection clear");

    assert!(discover_members(&mut pangine, &collection, "collection-member").is_empty());
    assert!(query_records(&mut pangine, &collection).is_empty());
    assert!(query_values_in_category(&mut pangine, &collection, "category-shared").is_empty());
}

#[test]
#[ignore = "warning: represented membership is payload-independent, but the application still owns its update lifecycle"]
fn represented_membership_does_not_depend_on_member_payload_shape_or_vocabulary() {
    let mut pangine = Pangine::new();
    let collection = pangine.reference_percept("opaque-collection");
    let member_a = pangine.reference_percept("opaque-member-a");
    let member_b = pangine.reference_percept("opaque-member-b");
    let atomic = must_ref(&mut pangine, "[atomic-payload]");
    let structured = must_ref(&mut pangine, "([left]->[one])([right]->[two])");
    let membership = collection_membership(&mut pangine, "mauve-link", &[member_a.clone(), member_b.clone()]);

    pangine
        .set_percept_values(&[(collection.clone(), Some(membership)), (member_a.clone(), Some(atomic)), (member_b.clone(), Some(structured))])
        .expect("valid opaque collection");

    assert_eq!(discover_members(&mut pangine, &collection, "mauve-link"), BTreeSet::from([member_a, member_b]));
}

fn initial_records() -> BTreeSet<CurrentRecord> {
    BTreeSet::from([
        CurrentRecord {
            record: "record-a".to_owned(),
            category: "category-shared".to_owned(),
            key: "key-a".to_owned(),
            value: "value-old".to_owned(),
            version: "version-one".to_owned(),
        },
        CurrentRecord {
            record: "record-b".to_owned(),
            category: "category-shared".to_owned(),
            key: "key-b".to_owned(),
            value: "value-stable".to_owned(),
            version: "version-one".to_owned(),
        },
    ])
}

fn record(pangine: &mut Pangine, record: &str, category: &str, key: &str, value: &str, version: &str) -> ConceptId {
    must_ref(
        pangine,
        &format!(
            "([{record}]->[collection-category]->[{category}])\
             ([{record}]->[collection-key]->[{key}])\
             ([{record}]->[collection-value]->[{value}])\
             ([{record}]->[collection-version]->[{version}])"
        ),
    )
}

fn collection_membership(pangine: &mut Pangine, relationship: &str, members: &[ConceptId]) -> ConceptId {
    let represented_members = members.iter().map(|member| format!("([{relationship}]->{})", pangine.format_concept(member, false))).collect::<String>();

    must_ref(pangine, &represented_members)
}

fn discover_members(pangine: &mut Pangine, collection: &ConceptId, relationship: &str) -> BTreeSet<ConceptId> {
    let member = pangine.reference_percept("discovered-member");
    let question = must_ref(pangine, &format!("[{relationship}]->['discovered-member']"));
    let result = pangine.complete_question(std::slice::from_ref(collection), &question).expect("valid represented membership question");

    assert!(
        result.completions().iter().flat_map(|completion| completion.evidence()).all(|evidence| evidence.source_percept() == Some(collection)),
        "membership discovery must retain the collection as its evidence source"
    );

    let members = result
        .completions()
        .iter()
        .map(|completion| completion.binding(&member).and_then(|binding| pangine.get_percept(binding)).expect("represented member Percept"))
        .collect::<BTreeSet<_>>();

    assert_eq!(result.completions().len(), members.len(), "represented membership must not return duplicate member identities");
    members
}

fn query_records(pangine: &mut Pangine, collection: &ConceptId) -> BTreeSet<CurrentRecord> {
    let members = discover_members(pangine, collection, "collection-member").into_iter().collect::<Vec<_>>();

    if members.is_empty() {
        return BTreeSet::new();
    }

    let question = must_ref(
        pangine,
        "(['record']->[collection-category]->['category'])
         (['record']->[collection-key]->['key'])
         (['record']->[collection-value]->['value'])
         (['record']->[collection-version]->['version'])",
    );
    let result = pangine.complete_question(&members, &question).expect("valid complete collection question");
    let records = records(pangine, &result);

    assert!(result
        .completions()
        .iter()
        .flat_map(|completion| completion.evidence())
        .all(|evidence| { evidence.source_percept().is_some_and(|source| members.iter().any(|member| member == source)) }));
    assert_eq!(result.completions().len(), records.len(), "complete records must not collapse duplicate completions");
    records
}

fn records(pangine: &mut Pangine, result: &CompletionResult) -> BTreeSet<CurrentRecord> {
    let record = pangine.reference_percept("record");
    let category = pangine.reference_percept("category");
    let key = pangine.reference_percept("key");
    let value = pangine.reference_percept("value");
    let version = pangine.reference_percept("version");

    result
        .completions()
        .iter()
        .map(|completion| CurrentRecord {
            record: bound_name(pangine, completion.binding(&record), "record"),
            category: bound_name(pangine, completion.binding(&category), "category"),
            key: bound_name(pangine, completion.binding(&key), "key"),
            value: bound_name(pangine, completion.binding(&value), "value"),
            version: bound_name(pangine, completion.binding(&version), "version"),
        })
        .collect()
}

fn query_values_in_category(pangine: &mut Pangine, collection: &ConceptId, category: &str) -> BTreeSet<(String, String, String)> {
    let members = discover_members(pangine, collection, "collection-member").into_iter().collect::<Vec<_>>();

    if members.is_empty() {
        return BTreeSet::new();
    }

    let question = must_ref(
        pangine,
        &format!(
            "(['record']->[collection-category]->[{category}])
             (['record']->[collection-key]->['key'])
             (['record']->[collection-value]->['value'])"
        ),
    );
    let result = pangine.complete_question(&members, &question).expect("valid exact category filter");
    let record = pangine.reference_percept("record");
    let key = pangine.reference_percept("key");
    let value = pangine.reference_percept("value");
    let records = result
        .completions()
        .iter()
        .map(|completion| {
            (
                bound_name(pangine, completion.binding(&record), "record"),
                bound_name(pangine, completion.binding(&key), "key"),
                bound_name(pangine, completion.binding(&value), "value"),
            )
        })
        .collect::<BTreeSet<_>>();

    assert_eq!(result.completions().len(), records.len(), "exact filter must not collapse duplicate completions");
    records
}

fn bound_name(pangine: &Pangine, value: Option<&ConceptId>, field: &str) -> String {
    pangine.get_name(value.unwrap_or_else(|| panic!("missing {field} binding"))).unwrap_or_else(|| panic!("unnamed {field} binding")).to_owned()
}

fn must_ref(pangine: &mut Pangine, input: &str) -> ConceptId {
    pangine
        .reference_concept(input)
        .unwrap_or_else(|error| panic!("failed to parse {input:?}: {error}"))
        .unwrap_or_else(|| panic!("expected non-null concept for {input:?}"))
}

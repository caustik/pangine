//! Warning check for replaceable current records supplied as an explicit collection.
//!
//! Each record lives in one Percept, so assignment replaces that record and a
//! missing value clears it. The caller still has to retain every member Percept
//! and supply the complete collection to each question. Pangine does not
//! currently discover collection members, enforce logical keys, persist them,
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
#[ignore = "warning: current records still require an external member registry and have no logical keys, arbitrary values, persistence, or transaction rollback"]
fn explicit_members_support_complete_query_filter_replace_and_clear() {
    let mut pangine = Pangine::new();
    let member_a = pangine.reference_percept("collection-member-a");
    let member_b = pangine.reference_percept("collection-member-b");
    let members = vec![member_a.clone(), member_b.clone()];

    let initial_a = record(&mut pangine, "record-a", "category-shared", "key-a", "value-old", "version-one");
    let initial_b = record(&mut pangine, "record-b", "category-shared", "key-b", "value-stable", "version-one");
    pangine.set_percept_values(&[(member_a.clone(), Some(initial_a)), (member_b.clone(), Some(initial_b))]).expect("valid initial collection");

    assert_eq!(query_records(&mut pangine, &members), initial_records());
    assert_eq!(
        query_values_in_category(&mut pangine, &members, "category-shared"),
        BTreeSet::from([
            ("record-a".to_owned(), "key-a".to_owned(), "value-old".to_owned()),
            ("record-b".to_owned(), "key-b".to_owned(), "value-stable".to_owned()),
        ])
    );
    assert!(query_values_in_category(&mut pangine, &members, "category-other").is_empty());

    let replacement_a = record(&mut pangine, "record-a", "category-shared", "key-a", "value-new", "version-two");
    pangine.set_percept_values(&[(member_a.clone(), Some(replacement_a))]).expect("valid replacement");

    assert_eq!(
        query_records(&mut pangine, &members),
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

    pangine.set_percept_values(&[(member_b, None)]).expect("valid clear");

    assert_eq!(
        query_records(&mut pangine, &members),
        BTreeSet::from([CurrentRecord {
            record: "record-a".to_owned(),
            category: "category-shared".to_owned(),
            key: "key-a".to_owned(),
            value: "value-new".to_owned(),
            version: "version-two".to_owned(),
        }])
    );
    assert_eq!(
        query_values_in_category(&mut pangine, &members, "category-shared"),
        BTreeSet::from([("record-a".to_owned(), "key-a".to_owned(), "value-new".to_owned())])
    );
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

fn query_records(pangine: &mut Pangine, members: &[ConceptId]) -> BTreeSet<CurrentRecord> {
    let question = must_ref(
        pangine,
        "(['record']->[collection-category]->['category'])
         (['record']->[collection-key]->['key'])
         (['record']->[collection-value]->['value'])
         (['record']->[collection-version]->['version'])",
    );
    let result = pangine.complete_question(members, &question).expect("valid complete collection question");
    let records = records(pangine, &result);

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

fn query_values_in_category(pangine: &mut Pangine, members: &[ConceptId], category: &str) -> BTreeSet<(String, String, String)> {
    let question = must_ref(
        pangine,
        &format!(
            "(['record']->[collection-category]->[{category}])
             (['record']->[collection-key]->['key'])
             (['record']->[collection-value]->['value'])"
        ),
    );
    let result = pangine.complete_question(members, &question).expect("valid exact category filter");
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

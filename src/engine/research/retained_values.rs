use super::super::Pangine;
use std::env;
use std::time::{Duration, Instant};

const DEFAULT_RETAINED_VALUE_SIZES: &str = "1000,10000,30000";

#[test]
#[ignore = "manual Release-mode retained experience value scaling report"]
fn reports_retained_value_scaling() {
    println!("retained_values,experiences,total_ms,microseconds_per_experience");
    for experiences in configured_sizes() {
        let elapsed = measure(experiences);
        println!("retained_values,{experiences},{:.3},{:.3}", milliseconds(elapsed), elapsed.as_secs_f64() * 1_000_000.0 / experiences as f64);
    }
}

fn configured_sizes() -> Vec<usize> {
    let configured = env::var("PANGINE_RETAINED_VALUE_SIZES").unwrap_or_else(|_| DEFAULT_RETAINED_VALUE_SIZES.to_owned());
    let mut sizes = configured
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.parse::<usize>().unwrap_or_else(|_| panic!("PANGINE_RETAINED_VALUE_SIZES contains invalid size {value:?}")))
        .collect::<Vec<_>>();
    assert!(!sizes.is_empty(), "PANGINE_RETAINED_VALUE_SIZES must contain at least one size");
    assert!(sizes.iter().all(|&size| size > 0), "PANGINE_RETAINED_VALUE_SIZES sizes must be positive");
    sizes.sort_unstable();
    sizes.dedup();
    sizes
}

fn measure(experiences: usize) -> Duration {
    let mut pangine = Pangine::new();
    let percept = pangine.reference_percept("memory");
    let mut returns = Vec::with_capacity(experiences);

    let start = Instant::now();
    for index in 0..experiences {
        let item = pangine.reference_named(&format!("item-{index}")).unwrap();
        let answer = pangine.reference_named(&format!("answer-{index}")).unwrap();
        let root = pangine.reference_ordered(vec![item, answer]);
        pangine.record_experience(&percept, &root).unwrap();
        returns.push(pangine.materialize_percept_value(&percept).unwrap());
    }
    let elapsed = start.elapsed();

    assert_eq!(returns.len(), experiences);
    assert_eq!(pangine.get_percept_roots(&percept).map(|roots| roots.len()), Some(experiences));
    let first_item = pangine.reference_named("item-0").unwrap();
    let first_answer = pangine.reference_named("answer-0").unwrap();
    let first_root = pangine.reference_ordered(vec![first_item, first_answer]);
    assert_eq!(returns.first(), Some(&first_root));
    assert_eq!(returns.last(), pangine.get_value(&percept).as_ref());
    elapsed
}

fn milliseconds(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

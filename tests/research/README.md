# Pangine research warning checks

The checks in this directory preserve useful behavior examples without declaring their exact outputs to be permanent Pangine semantics. They are compiled but ignored during the normal test run, which makes an unresolved boundary visible without blocking an intentional experiment.

Run every integration warning check explicitly with:

```sh
cargo test --test research --release -- --ignored
```

The current groups are:

- `question_support.rs`: how direct Percept-member relevance is currently projected into answer coefficients;
- `decision_fallback.rs`: the placeholder positive filter and canonical tie rule behind `^`; and
- `matcher_boundaries.rs`: current selector, nesting, and inferred-answer restrictions.

The accepted, application-independent completion behavior is in the ordinary suite at `pangine/tests/completion_questions.rs`. Syllogism-shaped composition, Rule 110, symbolic residual, correlation, and evidence-factor cases are kept together there because they are compatibility probes of one production evaluator, not separate research matchers.

One ignored internal check applies an external product oracle to retained completion factors:

```sh
cargo test --lib completion_calculus --release -- --ignored
```

That check demonstrates retained information, not Pangine probability semantics. A failure in any warning check means that provisional behavior changed. Review the input and the new behavior before deciding whether the implementation, the check, or both should change.

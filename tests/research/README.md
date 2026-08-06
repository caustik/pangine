# Pangine research warning checks

The checks in this directory preserve useful behavior examples without declaring their exact outputs to be permanent Pangine semantics. They are compiled but ignored during the normal test run, which makes the unresolved boundary visible without blocking an intentional experiment.

Run every warning check explicitly with:

```sh
cargo test --test research --release -- --ignored
```

The current groups are:

- `contextual_questions.rs`: which represented graph connections make an indirect candidate eligible;
- `question_support.rs`: how exact-root occurrence counts are currently projected into answer coefficients;
- `decision_fallback.rs`: the placeholder positive filter and canonical tie rule behind `^`;
- `matcher_boundaries.rs`: restrictions and inferred matches that were implemented before their wider meaning was understood.

A failure here means that a provisional example changed. Review the input and the new behavior before deciding whether the implementation, the check, or both should change. Do not promote one of these checks into the normal suite merely because its current output seems reasonable.

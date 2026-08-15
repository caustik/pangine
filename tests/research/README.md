# Pangine research warning checks

These ignored checks preserve unresolved comparisons without making their exact outputs permanent Pangine behavior.

Run them explicitly with:

```sh
cargo test --test research --release -- --ignored
cargo test --lib source_state_copy --release -- --ignored
cargo test --lib answer_adjustment_views --release -- --ignored
```

The two library commands run internal engine probes. They add no public snapshot, answer-view, or adjustment syntax.

Research programs that compare independent views copy a `$` result into a detached Percept before using `^`. Directly choosing a question output now conditions every output linked to that question.

## File index

- `action_loop.rs` forms and chooses a complete two-step route, continues from an observed position, and compares step evidence with complete-route experience without selecting a new Relevance rule.
- `application_choice.rs` compares two application-side rules with Pangine's current additive result. The rules can reject a larger total or abstain, but the fixtures do not establish that the application should own decisions.
- `decision_contract.rs` compares addition, multiplication, rescaling, ties, and distinct source histories. It keeps the information a future decision contract may need without choosing one formula.
- `decision_fallback.rs` records the current positive filter and canonical tie rule behind `^`.
- `decision_record.rs` compares saved totals, complete rows, evaluated values, and unchanged source Percepts. Each preserves a different part of an old decision.
- `experience_guided_decision.rs` keeps three troubleshooting decisions linked while helpful and failed episode answers adjust matching rows. It preserves the raw sources, leaves an untried decision available, and treats the outcome policy as provisional.
- `interface_percepts.rs` exercises complete Rust input groups, assigned-input experience capture, output delivery, and a queued later cycle. It adds no callback registry, event loop, or LLM adapter.
- `joint_answer_relevance.rs` keeps the current source-deduplication rule visible without treating additive integer support as the final Relevance model.
- `matcher_boundaries.rs` keeps four open matcher questions: ordered nesting, valid `@` subjects, unseen wholes assembled from partial experience, and enclosing-entry correlation.
- `outcome_learning.rs` compares actual transitions with identified episode outcomes, keeps untried routes through repeated regenerated detached choices, and preserves the old literal-adjustment boundary.
- `question_support.rs` records how direct Percept-member weights currently reach output coefficients.
- `represented_choice.rs` keeps focused counterexamples for experience, current state, context, stance, question order, source identity, records, and coefficients without host-side scoring.
- `row_choice.rs` shows that collapsing complete rows into totals can discard information needed by some decisions.
- `src/engine/research/source_state_copy.rs` compares value copies, live references, direct source-state copies, and represented version scopes. The behavior is test-only and does not choose a public lifecycle.
- `src/engine/research/answer_adjustment_views.rs` exercises the production immutable Answer and AnswerView API across explicit projections, collapse branches, adjustment receipts, strict publication, repeated outcomes, live-state boundaries, and weighted sources. It keeps unresolved edge semantics and a parser-level `@+=` / `@-=` candidate under warnings; it adds no public syntax.
- `src/engine/research/answer_adjustment_views/higher_order_adjustment.rs` composes candidate, outcome, and reliability Answers through the production API. It probes explicit order, branching, intermediate choice, duplicate paths, signs, cycles, flattened history, and linear source context through an eight-layer chain. It adds no public syntax.

Accepted behavior belongs in ordinary tests:

- `tests/answer_cycles.rs` covers repeated outcome-guided choices, compact possibility inspection, and the same cycle over an unordered three-output shape.
- `tests/completion_questions.rs` covers the current structural evaluator and correlated results.
- `tests/joint_answers.rs` covers visible shared answer shapes, answer extension, conditioning, subset choice, order effects, and answer-state detachment.
- `tests/percept_integration.rs` covers grouped input validation, assigned-input capture, stable experience, and the Rust-input-to-Pangine-output cycle.

Former projection, annotation, reduction, and successive decision-pipeline fixtures were removed after their distinct conclusions were summarized and kept in smaller warning checks. They were test-local experiments, not production behavior.

An ignored check failing after a deliberate experiment is a prompt to review the example. It is not automatic proof that the new behavior is wrong.

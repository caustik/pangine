# Pangine research warning checks

These ignored checks preserve unresolved comparisons without making their exact outputs permanent Pangine behavior.

Run them explicitly with:

```sh
cargo test --test research --release -- --ignored
cargo test --test research consumer_comparison --release -- --ignored --nocapture
cargo test --lib source_state_copy --release -- --ignored
cargo test --lib answer_adjustment_views --release -- --ignored
cargo test --lib answer_context --release -- --ignored
cargo test --lib structural_scope --release -- --ignored
cargo test --lib staged_inquiry --release -- --ignored --nocapture
cargo test --lib answer_lifecycle --release -- --ignored --nocapture
cargo test --lib answer_replay --release -- --ignored --nocapture
cargo test --lib concept_answer --release
```

The library commands run internal engine probes. They add no public snapshot or immutable Answer naming syntax.

The lifecycle report uses checkpoints `10,100,1000` by default. Set `PANGINE_ANSWER_CYCLE_SIZES` to a comma-separated list of positive cycle counts to run a smaller or larger manual report.

The replay report uses the same default checkpoints. Set `PANGINE_ANSWER_REPLAY_SIZES` to compare full-history rebuilds with a carried open Answer over another range. The same command runs a fixed replacement/retraction check whose size is not controlled by that variable.

Research programs that compare independent views copy a `$` result into a detached Percept before using `^`. Directly choosing a question output now conditions every output linked to that question.

## File index

- `action_loop.rs` forms and chooses a complete two-step route, continues from an observed position, and compares step evidence with complete-route experience without selecting a new Relevance rule.
- `application_choice.rs` compares two application-side rules with Pangine's current additive result. The rules can reject a larger total or abstain, but the fixtures do not establish that the application should own decisions.
- `consumer_comparison.rs` gives Pangine and ordinary Rust records the same evidence-review and failed-inquiry tasks. It compares changed-question results, retained witnesses, consumer conventions, implementation code, and bounded retained/returned text sizes without adding an executor or public syntax.
- `decision_contract.rs` compares addition, multiplication, rescaling, ties, and distinct source histories. It keeps the information a future decision contract may need without choosing one formula.
- `decision_fallback.rs` records the current positive filter and canonical tie rule behind `^`.
- `decision_record.rs` compares saved totals, complete rows, evaluated values, and unchanged source Percepts. Each preserves a different part of an old decision.
- `evidence_inquiry.rs` supplies a small, deliberately lossy evidence view through Rust, then uses ordinary questions to relate a completed result to observed and question-supplied structure. It compares recursive and structurally constrained source inspection, links whole records to nested fields through represented identity, and exposes the singleton and repeated-label boundaries of an open record pattern. Its field names have no engine meaning and are not a proposed Answer schema.
- `experience_guided_decision.rs` keeps troubleshooting decisions linked while outcome and review Answers adjust matching rows. It preserves raw sources, compares early and late choice across changing inputs, distinguishes weighted result Percepts from fixed result filters, and treats the outcome policy as provisional.
- `interface_percepts.rs` exercises complete Rust input groups, assigned-input experience capture, output delivery, and a queued later cycle. It adds no callback registry, event loop, or LLM adapter.
- `joint_answer_relevance.rs` keeps the current source-deduplication rule visible without treating additive integer support as the final Relevance model.
- `matcher_boundaries.rs` keeps open questions around ordered nesting, literal Percepts in structural `@` subjects, and enclosing-entry correlation.
- `outcome_learning.rs` compares actual transitions with identified episode outcomes, keeps untried routes through repeated regenerated detached choices, and preserves the old literal-adjustment boundary.
- `question_support.rs` records how direct Percept-member weights currently reach output coefficients.
- `recursive_inquiry.rs` uses `recursive_inquiry.pae` as one observation corpus for direct inspection, further questions, explicit alternative conclusions, source remainders, question-shape inspection, and an Answer about an Answer. Codec inspection is explicitly a Rust-assisted probe, not new public evidence syntax.
- `reference_inquiry.rs` compares literal references, singleton question shapes, ordinary input constraints, detached aliases, and mixed linked/ordinary reads. Two test-only evaluation rules repair different examples but both retain the mixed-read pairing problem; neither is selected as the meaning of `$`.
- `represented_choice.rs` keeps focused counterexamples for experience, current state, context, stance, question order, source identity, records, and coefficients without host-side scoring.
- `row_choice.rs` shows that collapsing complete rows into totals can discard information needed by some decisions.
- `src/engine/research/source_state_copy.rs` compares value copies, live references, direct source-state copies, and represented version scopes. The behavior is test-only and does not choose a public lifecycle.
- `src/engine/research/answer_adjustment_views.rs` exercises the production immutable Answer and AnswerView API and the public `@+=` / `@-=` operations across explicit projections, collapse branches, adjustment receipts, strict publication, repeated outcomes, live-state boundaries, and weighted sources. It keeps deeper composition and policy questions under warnings.
- `src/engine/research/answer_adjustment_views/higher_order_adjustment.rs` composes candidate, outcome, and reliability Answers through the production API. It probes explicit order, branching, intermediate choice, duplicate paths, signs, cycles, flattened history, and linear source context through an eight-layer chain. It adds no public syntax.
- `src/engine/research/answer_context.rs` exercises the existing completion join with an ordinary atomic input, independent Answers, and shared bindings. It checks source preservation and bounded associativity of complete evidence, while exposing why querying a compound value is different from binding that whole value. It proposes no public join or context syntax.
- `src/engine/research/structural_scope.rs` starts the existing matcher at a whole value or its direct union members. It compares those scopes with recursive discovery, binds compound/reference/Answer inputs before joining, tests existing Answer transport, and inspects fields across absent, singleton, and larger records while retaining their complete source. Coefficients remain structural and question remainders remain provisional; the helpers are test-only semantic comparisons.
- `src/engine/research/staged_inquiry.rs` asks inside a directly bound value while retaining parent pairings and original sources. It exposes a lost selected-position distinction and the inability to select a compound projection. Test-only records of subject, question, and Answer preserve the request and support recursive inspection and captured replay across engines through eight further questions, while reporting eager transport size. Neither the restricted executor nor the tuple convention is a public interface.
- `src/engine/research/answer_lifecycle.rs` reports Concept count, encoded answer size, proof rows and fragments, source visits, inspection size, and revisions across repeated current-grammar answer cycles. Its helpful-minus-failed roles exercise existing explicit operations rather than defining an outcome or Relevance policy.
- `src/engine/research/answer_replay.rs` compares a full-history rebuild with applying only each newest stable episode source to a carried open Answer. It requires exact proof-bearing and choice equivalence for append-only sources, then shows that inverse adjustment after replacement or retraction preserves visible results but retains cancelling proof and increases encoded state. It adds no public syntax or state.
- `src/engine/concept_answer.rs` retains production answers as ordinary Concepts. Its focused tests exercise the codec, production-backed projection, collapse, adjustment, and joining, detachment, cross-engine round trips, indexed matching, and deterministic partition reduction.

Broad current capabilities belong in ordinary tests:

- `tests/answer_cycles.rs` covers repeated outcome-guided choices, compact possibility inspection, and the same cycle over an unordered three-output shape.
- `tests/completion_questions.rs` covers the current structural evaluator and correlated results.
- `tests/joint_answers.rs` covers visible shared answer shapes, answer extension, conditioning, subset choice, order effects, and detachment from a shared answer.
- `tests/percept_selectors.rs` proves selector meaning does not depend on a Percept's value shape and that the read-only global Percept follows the same source-selection path as other Percepts.
- `tests/structural_subjects.rs` compares direct and retained reference-bearing Concepts through eight nesting levels, re-questions question shapes, distinguishes literal references from explicit evaluation, and retains foreign-engine validation.
- `tests/value_roundtrips.rs` checks that materialized coefficient-bearing results follow literal union construction while their complete Answers preserve both possibilities through cross-engine transport, including an empty projection.
- `tests/concept_lifetime.rs` covers the global view's lifetime, read-only behavior, raw-Percept exclusion, and retention of ordinary Concepts that contain Percept references.
- `tests/percept_integration.rs` covers grouped input validation, assigned-input capture, stable experience, and the Rust-input-to-Pangine-output cycle.

Former projection, annotation, reduction, and successive decision-pipeline fixtures were removed after their distinct conclusions were summarized and kept in smaller warning checks. They were test-local experiments, not production behavior.

An ignored check failing after a deliberate experiment is a prompt to review the example. It is not automatic proof that the new behavior is wrong.

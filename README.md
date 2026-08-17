# Pangine

Pangine is an experimental language for writing information as simple shapes and asking questions of those shapes.

Created by [Aaron (`caustik`)](https://github.com/caustik) and released by APU Software, LLC.

I started Pangine from one intuition: the information, the question, and the answer should be made from the same thing. In Pangine, that thing is a Concept.

Pangine is not a trained model and it does not know what names mean. It works with the structure and experience you give it.

## Pangine in one minute

Remember two statements and ask a question:

```text
command> ['memory'] ~= [cat]->[purrs]
  {[cat]->[purrs]}
command> ['memory'] ~= [dog]->[barks]
  {[cat]->[purrs]}
  {[dog]->[barks]}
command> ['memory'] @ ['animal']->['sound']
  {[cat]->[purrs]}
  {[dog]->[barks]}
command> $['animal']
  [cat]
  [dog]
command> $['sound']
  [barks]
  [purrs]
```

`[cat]` is a named Concept. `[cat]->[purrs]` is an ordered Concept. `['memory']` is a Percept, which is Pangine's mutable reference. `~=` remembers one complete experience under it.

`@` asks a question. The Percepts inside the question are blanks to fill. Its immediate result contains the complete rows, so Pangine keeps `cat` with `purrs` and `dog` with `barks`. `$` reads any part of that answer without changing it.

## Concepts and questions

The same grammar describes information, questions, and answers:

```text
[cat]
[cat]->[purrs]
[cat][dog]
([person]->[Alice])([pet]->[cat])
```

A grounded answer is an ordinary Concept again. It can be assigned, formatted, parsed, or used as the subject of another question.

Several relationships can form one question. Reusing a Percept connects their blanks:

```text
['knowledge'] ~= [Socrates]->[is-a]->[human]
['knowledge'] ~= [human]->[is-a]->[mortal]
['knowledge'] @ ([Socrates]->[is-a]->['kind'])(['kind']->[is-a]->['conclusion'])
$['conclusion']
```

The result is `[mortal]`. Pangine does not know that `is-a` is logical. The question asks for two relationships whose middle Concept must agree.

Parentheses preserve a complete unordered member. This keeps alternatives such as `([person]->[Alice])([pet]->[cat])` together. `*` explicitly merges direct members when they are meant to share one pool. The matcher also tracks represented occurrences while a question runs, so equal values in different parts of a longer source do not create unsupported cross-pairings. This temporary bookkeeping is not added to the returned Concept.

## Experience and choice

Repeating an experience raises its current integer weight:

```text
['world'] ~= [morning]->[birds]
['world'] ~= [morning]->[birds]
['world'] ~= [morning]->[traffic]
['world'] @ [morning]->['answer']
$['answer']
```

The last command shows `x2 [birds]` and `[traffic]`. `x2 [birds]` is the compact form of two equal bird members. Pangine currently exposes remembered support this way.

`^['answer']` chooses the greatest positive weight and uses canonical order to break a tie. This is a deterministic placeholder, not probability, confidence, sampling, or a finished theory of Relevance.

I think of `@` as leaving possible answers together and `^` as collapsing them to one represented answer. Experience is allowed to shape that choice. The application supplies observations and current values, but the Pangine program should form and choose among candidates instead of hiding that decision in application code.

## Shared answers

Outputs from one question stay connected to the same complete answer. `&` reveals that answer's question shape, `$` reads it, and `^` removes complete rows that do not fit the chosen result.

Suppose the memory contains `cat-fish` once, `cat-milk` twice, and `dog-fish` three times:

```text
['memory'] ~= [cat]->[fish]
['memory'] ~= [cat]->[milk]
['memory'] ~= [cat]->[milk]
['memory'] ~= [dog]->[fish]
['memory'] ~= [dog]->[fish]
['memory'] ~= [dog]->[fish]
['memory'] @ ['animal']->['food']
```

The linked answer can then be inspected and changed:

```text
command> &['animal']
  {['animal']->['food']}
command> $(&['animal'])
  x3 {[dog]->[fish]}
  x2 {[cat]->[milk]}
  {[cat]->[fish]}
command> ^['animal']
  [cat]
command> $['food']
  x2 [milk]
  [fish]
```

Choosing `animal` removes the `dog-fish` row, then recalculates `food` from the surviving rows. Choosing several outputs together, such as `^(['animal']->['food'])`, chooses that complete subset at once. Separate choices can produce a different result because each choice changes what remains for the next one.

A later question can reuse one linked output. Pangine joins compatible old and new rows and expands the shared answer. If no row is compatible, it returns `[]` without changing the existing answers. Asking again with every output from one answer starts a new answer cycle.

Assignment detaches a value. For example, `['animal-copy'] = $['animal']` makes an independent copy that can be chosen without collapsing the original answer.

Two linked answers can also affect one another without being copied into ordinary values:

```text
['action']->['tool'] @+= ['helpful-action']->['helpful-tool']
['action']->['tool'] @-= ['failed-action']->['failed-tool']
```

These commands assume earlier questions filled the candidate, helpful, and failed Percepts. Each side names the part of one linked answer to compare. Matching helpful sources are added to the candidate rows, matching failed sources are subtracted, and the whole target answer receives a new revision. Only the target is published, so a separate source answer stays unchanged. Either side can be one Percept or a larger shape. An unlinked operand is an error. Ordinary `+=` and `-=` still change ordinary Percept values.

## Input Percepts

The console, pangine.com, and Rust can provide current values through Percepts. Assign the values, then mention them in an experience:

```text
['context-input'] = [opal]
['reading-input'] = [cedar]
['observations'] ~= [observation]->[context]->['context-input']->[reading]->['reading-input']
```

When `~=` runs, Pangine captures assigned Percepts at that moment. Later changes do not rewrite old experience. If a required input is empty, Pangine records nothing instead of keeping a partial observation.

A Percept populated through `~=` remains a reference when another experience mentions it. Use `$` when you want to follow every Percept in an expression. Rust callers can update a complete input group with `set_percept_values`, remember a Percept-bearing Concept with `perform_experience`, and read the resulting output Percepts.

## Grammar

| Form | Meaning |
| --- | --- |
| `[]` | No Concept |
| `[name]` | Named Concept |
| `['memory']` | Mutable Percept reference |
| `[A]->[B]->[C]` | Ordered Concept |
| `[A][B]` | Unordered Concept containing `A` and `B` |
| `(expression)` | Keep the expression as one surrounding member |
| `[A]*[B]` | Merge direct unordered members |
| `[A]/[B]` | Merge with an inverted right side |
| `![A]` | Inverted member |
| `x2[A]` | Two copies of the next complete member |
| `['memory'] = expression` | Replace a Percept value |
| `['memory'] += expression` | Add a value |
| `['memory'] -= expression` | Subtract a value |
| `['memory'] ~= expression` | Capture assigned inputs and remember one experience |
| `subject @ question` | Fill the question's Percept blanks |
| `['target'] @+= ['evidence']` | Add matching sources from another linked answer |
| `['target'] @-= ['evidence']` | Subtract matching sources from another linked answer |
| `&operand` | Return the shared answer shape |
| `$operand` | Read Percepts without changing their answer |
| `^operand` | Choose and update every linked output |
| `$['*']` | Inspect the ordinary Concepts currently live in the engine |

See [pangine.com/grammar.html](https://pangine.com/grammar.html) for the compact reference and [pangine.com/examples.html](https://pangine.com/examples.html) for literal console transcripts.

## Current scope

The Rust prototype includes the parser, canonical Concept graph, mutable Percepts, remembered experience, structural questions, correlated answer rows, visible shared answers, immutable Rust Answer values, collapse, grouped input updates, a console that can run commands interactively or from a file, a browser-local WebAssembly workbench, ordinary tests, and focused research warnings.

The current signed integer and deterministic choice rule are useful placeholders. `AnswerView::possibilities` exposes each projected value, its current strength, complete-row count, distinct source contributions, and whether it shares the greatest positive strength. The complete rows and question shape remain available through the Answer itself.

When Pangine remembers or replaces experience, it records the recursively reachable shapes and required fixed names while keeping each complete source intact. A question uses those records to find possible source experiences before running the full matcher. It does not walk every remembered experience unless the question is broad enough to require them. Question parts can work together within one complete experience, while a repeated Percept can join separate experiences. Equal complete answers then combine support without mixing unrelated partial rows.

The Rust Answer API can branch, choose, adjust matching answer views, and explicitly publish a current revision. An adjusted Answer can be projected, chosen, or used to adjust another Answer, so additional layers use the same object and operation. The console exposes live target adjustment through `@+=` and `@-=`. Naming immutable Answer branches, compact source inspection, additional grammar, logit sampling, probabilities, persistence, automatic callbacks, broad bindings, a general LLM adapter, and a distributed runtime are not the current focus.

The ordinary answer-cycle checks now re-ask a complete action-tool decision after recording new outcomes. Two additional failures change the later choice while leaving every untried possibility and its sources available. The same operations also choose an unordered action-tool-scope shape without any single-Percept rule. This demonstrates the core cycle under one explicit outcome policy; it does not establish that policy as universal.

An LLM could eventually supply explicit structured records and questions or consume selected outputs. Pangine would keep the source boundaries, joins, experience, alternatives, and choice visible. The LLM or application should not silently become Pangine's relevance calculator or final judge.

## Run Pangine

Install a current stable Rust toolchain, then:

```sh
git clone https://github.com/caustik/pangine.git
cd pangine
cargo run --bin pangine-console
```

Run the checked-in decision programs without typing each command:

```sh
cargo run --bin pangine-console -- examples/route-cycle.pae
cargo run --bin pangine-console -- examples/settings-choice.pae
```

The route program re-asks three complete routes, adjusts the linked answer from recorded results, and chooses the complete route. The settings program keeps three outputs linked and chooses them together. An application can replace the input assignments and read the selected outputs without ranking the choices itself. These are capability examples, not fixed application areas.

Run the normal suite with:

```sh
cargo test --workspace --all-targets --release
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Ignored research tests record provisional questions, not compatibility promises.

## Contributing

Reproducible bug reports and focused design discussion are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md).

## Licensing

Pangine is source-available under the [PolyForm Noncommercial License 1.0.0](LICENSE.md). Noncommercial use, modification, and distribution are permitted under its terms; commercial use requires separate permission from APU Software, LLC.

This is not an OSI-approved open-source license. See [NOTICE](NOTICE) for ownership and attribution.

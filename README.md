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
| `['memory'] ~= expression` | Capture assigned inputs and remember one experience |
| `subject @ question` | Fill the question's Percept blanks |
| `&operand` | Return the shared answer shape |
| `$operand` | Read Percepts without changing their answer |
| `^operand` | Choose and update every linked output |
| `$['*']` | Inspect the ordinary Concepts currently live in the engine |

See [pangine.com/grammar.html](https://pangine.com/grammar.html) for the compact reference and [pangine.com/examples.html](https://pangine.com/examples.html) for literal console transcripts.

## Current scope

The Rust prototype includes the parser, canonical Concept graph, mutable Percepts, remembered experience, structural questions, correlated answer rows, visible shared answers, collapse, grouped input updates, a console that can run commands interactively or from a file, a browser-local WebAssembly workbench, ordinary tests, and focused research warnings.

The current signed integer and deterministic choice rule are useful placeholders. The open work is to compare better Relevance rules over the retained source evidence and learn how far complete shared answers can go in useful programs.

A learned decision step, logit sampling, probabilities, persistence, automatic callbacks, LLM adapters, and a distributed runtime are not implemented. An LLM could eventually be an identified source or output participant. It should not silently become Pangine's question selector, relevance calculator, or final judge.

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

The route program records two failures, changes from the east route to the north route, then strengthens north after a success. The settings program keeps three output Percepts linked, chooses them together, and returns one complete setting Concept. Both remember the supplied result. An application can replace the input assignments and read the selected outputs without ranking the choices itself.

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

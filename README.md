# Pangine

Pangine is an experimental language for writing experience as simple shapes and asking questions of it. It answers exactly from what it remembers, or with graded answers composed from parts and generalized from similar cases, and every answer keeps its sources.

Created by [Aaron (`caustik`)](https://github.com/caustik) and released by APU Software, LLC.

I started Pangine from one intuition: the information, the question, and the answer should be made from the same thing. In Pangine, that thing is a Concept.

Pangine is not a trained model and it does not know what names mean. It works with the structure and experience you give it.

## Pangine in one minute

Remember two statements and ask a question:

```text
command> {memory} ~= [cat]->[purrs]
  [cat]->[purrs]
command> {memory} ~= [dog]->[barks]
  [cat]->[purrs]
  [dog]->[barks]
command> {memory} @ {animal}->{sound}
  [cat]->[purrs]
  [dog]->[barks]
command> ${animal}
  [cat]
  [dog]
command> ${sound}
  [barks]
  [purrs]
```

`[cat]` is a named Concept. `[cat]->[purrs]` is an ordered Concept. `{memory}` is a Percept, which is Pangine's mutable reference. `~=` remembers one complete experience under it.

`@` asks a question. The Percepts inside the question are blanks to fill. Its immediate result contains the complete rows, so Pangine keeps `cat` with `purrs` and `dog` with `barks`. `$` reads any part of that answer without changing it.

## Concepts and questions

The same grammar describes information, questions, and answers:

```text
[cat]
["C:\\Music\\Track 01.wav"]
["{\"revision\":2}"]
[cat]->[purrs]
[cat][dog]
([person]->[Alice])([pet]->[cat])
```

Use `[name]` for names made from ASCII letters, digits, spaces, `_`, and `-`. Use `["escaped text"]` for any other UTF-8 text. Percepts use the parallel `{name}` and `{"escaped text"}` forms. Escaped text supports `\"`, `\\`, `\0`, `\b`, `\t`, `\n`, `\f`, `\r`, and `\u{...}`. Both Concept spellings create the same kind of opaque named Concept, so `["cat"]` formats canonically as `[cat]`.

`[]` remains no Concept. `[""]` is a normal named Concept whose name is the empty string, while an absent optional relationship is represented by leaving that relationship out. The corresponding Percept is `{""}`.

An answer is an ordinary Concept again. It can be assigned, formatted, parsed, or used as the subject of another question. A structural subject keeps any embedded Percepts as represented references. Use `$` explicitly when their current values are wanted instead. A plain Percept, or an unordered default-coefficient set of Percepts, selects the complete Concepts retained under those sources.

Several relationships can form one question. Reusing a Percept connects their blanks:

```text
{knowledge} ~= [Socrates]->[is-a]->[human]
{knowledge} ~= [human]->[is-a]->[mortal]
{knowledge} @ ([Socrates]->[is-a]->{kind})({kind}->[is-a]->{conclusion})
${conclusion}
```

The result is `[mortal]`. Pangine does not know that `is-a` is logical. The question asks for two relationships whose middle Concept must agree.

Parentheses preserve a complete unordered member. This keeps alternatives such as `([person]->[Alice])([pet]->[cat])` together. `*` explicitly merges direct members when they are meant to share one pool. Equal values at different positions remain distinct while matching.

## The global Percept

`{*}` is a read-only view of the ordinary Concepts currently live in the engine. It can be read with `$` or used as a question source:

```text
command> {memory} = [known]
  [known]
command> {*} @ {answer}
  [known]
```

Here the global view supplies `[known]` to the question. Reading it with `$` also follows any Percept references inside those Concepts.

## Experience and choice

Relevance counts evidence. Repeating an experience adds one to its count:

```text
{world} ~= [morning]->[birds]
{world} ~= [morning]->[birds]
{world} ~= [morning]->[traffic]
{world} @ [morning]->{answer}
${answer}
```

The last command shows `x2 [birds]` and `[traffic]`. `x2 [birds]` is the compact form of two equal bird members, one for each time that experience was remembered.

An answer reads these counts as probabilities. Each value's probability is its share of the positive evidence among the alternatives, so birds is 2/3 and traffic 1/3. The members of an unordered Concept read the same way: `x2[tea]x3[coffee]` is tea 2/5 and coffee 3/5. The probability is the relative frequency of what Pangine remembers, not a calibrated confidence that an answer is true.

A row joined from separate experiences weighs the product of their counts, the number of ways to assemble it from what Pangine remembers. If `{knowledge}` in the Socrates example above held `[Socrates]->[is-a]->[human]` three times, `mortal` would carry three units of evidence. One experience that proves several parts of a row was observed whole, so it counts once.

`^{answer}` chooses the most probable value, the one with the greatest positive count, and uses canonical order to break a tie. Evidence can be negative: an inverted member such as `![tea]` counts minus one, and `@-=` subtracts matching evidence from an answer. A value whose evidence is zero or negative has probability zero and is never chosen. When no value has positive evidence, `^` returns `[]`.

`^~{choice}` draws a value instead, with probability equal to its share, so tea comes up with probability 2/5 and coffee 3/5:

```text
command> {choice} = x2[tea]x3[coffee]
  x3 [coffee]
  x2 [tea]
command> ^{choice}
  [coffee]
command> ^~{choice}
  [coffee]
command> ^~{choice}
  [tea]
command> ^~{choice}
  [coffee]
command> seed 0
command> ^~{choice}
  [coffee]
command> ^~{choice}
  [tea]
```

Each engine draws from its own generator, which starts at seed 0, so the same commands draw the same values in every run. `seed 0` restarted that sequence above; the console's `seed n` command and Rust's `set_sample_seed` choose another one. On a linked answer, `^~` removes incompatible rows exactly as `^` does, and on a graded answer it draws by the interpolated probabilities described below. When no value has positive evidence, `^~` also returns `[]`.

I think of `@` as leaving possible answers together and `^` as collapsing them to one represented answer. Experience is allowed to shape that choice. A program can also inspect the possibilities, ask another question, or leave the answer open. A consumer's interpretation or choice policy should remain distinguishable from the behavior Pangine supplies.

Every selected source Percept on the left of `@` can add support to matching results. When a current value should only restrict the question, read it with `$` inside the question instead.

## Graded questions

`@` answers only from experience that fits the question as asked. `@~` asks the same question, then widens it when experience is thin, in two steps: parts of the question that share no blank may come from separate experiences, and a single remembered case may match the question's shape with some of its names changed.

```text
command> {closet} ~= ([top]->[red])([bottom]->[jeans])
  [bottom]->[jeans]
  [top]->[red]
command> {closet} ~= ([top]->[blue])([bottom]->[skirt])
  ([bottom]->[jeans])([top]->[red])
  ([bottom]->[skirt])([top]->[blue])
command> {closet} ~= [top]->[green]
  [top]->[green]
  ([bottom]->[jeans])([top]->[red])
  ([bottom]->[skirt])([top]->[blue])
command> {closet} @ ([top]->{shirt})([bottom]->{pants})
  ([bottom]->[jeans])([top]->[red])
  ([bottom]->[skirt])([top]->[blue])
command> {closet} @~ ([top]->{shirt})([bottom]->{pants})
  ([bottom]->[jeans])([top]->[blue])
  ([bottom]->[jeans])([top]->[green])
  ([bottom]->[jeans])([top]->[red])
  ([bottom]->[skirt])([top]->[blue])
  ([bottom]->[skirt])([top]->[green])
  ([bottom]->[skirt])([top]->[red])
command> $({shirt}->{pants})
  x4([blue]->[skirt])
  x4([red]->[jeans])
  [blue]->[jeans]
  [green]->[jeans]
  [green]->[skirt]
  [red]->[skirt]
```

`@` returns the two outfits that were worn whole. `@~` also composes outfits from tops and bottoms seen separately, including the green top that was never worn with anything. Its probabilities mix the two levels with Witten-Bell interpolation, a standard rule from language modeling that needs no tuning: the more evidence the exact answer has, relative to how many different answers it gives, the more it counts, and the rest falls through to the composed answers. Here each outfit worn whole keeps 1/3 and each composed one gets 1/12.

A graded `$` shows each probability as a share of a common denominator, so `x4` above means 4 of 12. `^` chooses the most probable value, and `^~` draws one with these probabilities.

A remembered case can also answer a question it does not match exactly:

```text
command> {families} ~= ([Tom]->[parent-of]->[Bob])([Bob]->[parent-of]->[Ann])([Tom]->[grandparent-of]->[Ann])
  [Bob]->[parent-of]->[Ann]
  [Tom]->[grandparent-of]->[Ann]
  [Tom]->[parent-of]->[Bob]
command> {families} ~= ([Liz]->[parent-of]->[Max])([Max]->[parent-of]->[Ivy])([Liz]->[grandparent-of]->[Ivy])
  ([Bob]->[parent-of]->[Ann])([Tom]->[grandparent-of]->[Ann])([Tom]->[parent-of]->[Bob])
  ([Liz]->[grandparent-of]->[Ivy])([Liz]->[parent-of]->[Max])([Max]->[parent-of]->[Ivy])
command> {families} @ ([Joe]->[parent-of]->[Sue])([Sue]->[parent-of]->[Kim])([Joe]->[grandparent-of]->{who})
  []
command> {families} @~ ([Joe]->[parent-of]->[Sue])([Sue]->[parent-of]->[Kim])([Joe]->[grandparent-of]->{who})
  x2(([Joe]->[grandparent-of]->[Kim])([Joe]->[parent-of]->[Sue])([Sue]->[parent-of]->[Kim]))
command> ${who}
  [Kim]
```

No family mentions Joe, so `@` finds nothing. `@~` compares the question with each remembered family as one complete case. The distance counts the name occurrences that differ, here Joe twice, Sue twice, and Kim once, plus one more for each extra value that a name the question repeats would take, so a match that split Joe into two different people would be farther away. In both families the grandchild sits where the question's `[Kim]` sits, so each family answers `[Kim]` in the question's own terms, and `@~` completes the whole family it never saw. A case counts only if it shares at least one name with the question, and each part of the question must come from a different part of the case. Only names inside ordered relationships can change; a name that is itself a member of an unordered group still matches by identity.

Each distance forms one more level below the composed answers, and the same interpolation runs down every level. `inspect` marks composed rows with the separate experiences they came from and generalized rows with their distance and case. When nothing fits at any level, `@~` returns `[]`. A graded question with only exact rows answers exactly like `@`.

Probabilities stay exact fractions while their arithmetic fits in 128 bits, which covers every example here. A large graded answer can need more. When `$` would need shares too large for its 64-bit evidence counts, it shows millionths, rounded to add up to one million, and past 128 bits the interpolation continues in fixed point. `inspect` prints a probability as a fraction, or as a six-place decimal when its denominator is above one million or it was computed in fixed point. Every step uses integer arithmetic, so the console and the browser compute the same values.

## Shared answers

Outputs from one question stay connected to the same complete answer. `&` reveals that answer's question shape, `$` reads it, and `^` removes complete rows that do not fit the chosen result.

Every linked output stores that answer as the same ordinary versioned Concept. Rust can retrieve or install the Concept with `linked_answer_value` and `install_answer_value`.

Suppose the memory contains `cat-fish` once, `cat-milk` twice, and `dog-fish` three times:

```text
{memory} ~= [cat]->[fish]
{memory} ~= [cat]->[milk]
{memory} ~= [cat]->[milk]
{memory} ~= [dog]->[fish]
{memory} ~= [dog]->[fish]
{memory} ~= [dog]->[fish]
{memory} @ {animal}->{food}
```

The linked answer can then be inspected and changed:

```text
command> &{animal}
  {animal}->{food}
command> $(&{animal})
  x3([dog]->[fish])
  x2([cat]->[milk])
  [cat]->[fish]
command> ^{animal}
  [cat]
command> ${food}
  x2 [milk]
  [fish]
```

Choosing `animal` removes the `dog-fish` row, then recalculates `food` from the surviving rows. Choosing several outputs together, such as `^({animal}->{food})`, chooses that complete subset at once. Separate choices can produce a different result because each choice changes what remains for the next one.

A later question can reuse one linked output. Pangine joins compatible old and new rows and expands the shared answer. If no row is compatible, it returns `[]` without changing the existing answers. Asking again with every output from one answer starts a new answer cycle.

A question shape can itself be inspected with another question:

```text
command> (&{animal}) @ {left}->{right}
  {animal}->{food}
command> ${left}
  {animal}
command> ${right}
  {food}
```

Here the subject contains Percept references, and the new blanks capture those references. The original animal-food answer stays open. Reading a linked output substitutes its binding once; another `$` can then read the captured reference's current value. Ordinary detached values are followed recursively. These evaluation boundaries remain prototype behavior. [`examples/question-inquiry.pae`](examples/question-inquiry.pae) continues the inspection through another question using the same operations.

Materialized results follow the same coefficient composition as written Concepts. The current wildcard question `x2[A] @ {part}` finds both `x2[A]` and its inner `[A]`; their ordinary union is `x3[A]`. The linked Answer still retains both complete possibilities. Inverted members can cancel in a materialized result without erasing its linked Answer.

Assignment detaches a value. For example, `{animal-copy} = ${animal}` makes an independent copy that can be chosen without collapsing the original answer.

Two linked answers can also affect one another without being copied into ordinary values:

```text
{action}->{tool} @+= {helpful-action}->{helpful-tool}
{action}->{tool} @-= {failed-action}->{failed-tool}
```

These commands assume earlier questions filled the candidate, helpful, and failed Percepts. Each side names the part of one linked answer to compare. Matching helpful rows add their evidence to the candidate rows, matching failed rows subtract theirs, and every linked target output is updated. Only the target changes, so a separate source answer stays unchanged. Either side can be one Percept or a larger shape. An unlinked operand is an error. Ordinary `+=` and `-=` still change ordinary Percept values.

Evidence imported from a graded answer keeps its grade, so a case that only resembles the target counts below the target's own evidence. A graded answer cannot itself be adjusted, because one value can sit in rows of several grades, and each row would take the same evidence.

## Input Percepts

The console, pangine.com, and Rust can provide current values through Percepts. Assign the values, then mention them in an experience:

```text
{context-input} = [opal]
{reading-input} = [cedar]
{observations} ~= [observation]->[context]->{context-input}->[reading]->{reading-input}
```

When `~=` runs, Pangine captures assigned Percepts at that moment. Later changes do not rewrite old experience. If a required input is empty, Pangine records nothing instead of keeping a partial observation.

A Percept populated through `~=` remains a reference when another experience mentions it. Use `$` when you want to follow every Percept in an expression. Rust callers can update a complete input group with `set_percept_values`, remember a Percept-bearing Concept with `perform_experience`, and read the resulting output Percepts.

## Partitions

Remembered experience can be divided among partition engines, which is how I first thought about scaling Pangine. Each experience lives in the partition chosen by a hash of its canonical spelling, so repeated experience adds up where it lives. A question over a memory goes to every partition, each partition matches its own experience, and the engine reduces their partial answers. How the experience is divided does not change the answer:

```text
command> {world} ~= [morning]->[birds]
  [morning]->[birds]
command> {world} ~= [morning]->[birds]
  x2([morning]->[birds])
command> {world} ~= [morning]->[traffic]
  x2([morning]->[birds])
  [morning]->[traffic]
command> {world} ~= [evening]->[crickets]
  x2([morning]->[birds])
  [evening]->[crickets]
  [morning]->[traffic]
command> partitions 3
  3 partitions
  partition 0: no remembered experience
  partition 1: {world} 1 experience
  partition 2: {world} 2 experiences
command> {world} @ {time}->{sound}
  [evening]->[crickets]
  [morning]->[birds]
  [morning]->[traffic]
command> drop partition 2
  3 partitions
  partition 0: no remembered experience
  partition 1: {world} 1 experience
  partition 2: no remembered experience
command> {world} @ {time}->{sound}
  [evening]->[crickets]
```

`partitions n` divides the experience among `n` partitions, `partitions` lists what each one holds, and `partitions 1` brings everything back together. `drop partition k` loses one partition as if its machine were replaced. Answers already given stay, and later questions answer from the experience that remains, here the one experience in partition 1. The engine itself keeps current values, answers, and a copy of each memory for reading, and it rebuilds that copy from the remaining partitions after a loss.

In Rust, `set_partitions`, `partition_count`, `drop_partition`, and `partition_of` do the same. Native builds run each partition on its own thread, and the browser runtime runs them in turn. Dividing pays off when matching dominates. On a 16-core machine, a graded question over 8,000 remembered families took 717 ms in one engine and 375 ms across 8 partitions. A small exact question took 0.09 ms in one engine and 0.13 to 0.24 ms divided, because every question crosses to each partition and back. The partitions share one process; Pangine has no network protocol.

## Grammar

| Form | Meaning |
| --- | --- |
| `[]` | No Concept |
| `[name]` | Named Concept |
| `["escaped text"]` | Named Concept containing escaped UTF-8 text |
| `{memory}` | Mutable Percept reference |
| `{"escaped text"}` | Mutable Percept reference containing escaped UTF-8 text |
| `[A]->[B]->[C]` | Ordered Concept |
| `[A][B]` | Unordered Concept containing `A` and `B` |
| `(expression)` | Keep the expression as one surrounding member |
| `[A]*[B]` | Merge direct unordered members |
| `[A]/[B]` | Merge with an inverted right side |
| `![A]` | Inverted member |
| `x2[A]` | Two copies of the next complete member |
| `{memory} = expression` | Replace a Percept value |
| `{memory} += expression` | Add a value |
| `{memory} -= expression` | Subtract a value |
| `{memory} *= expression` | Merge direct members into the value |
| `{memory} /= expression` | Merge inverted direct members into the value |
| `{memory} ~= expression` | Capture assigned inputs and remember one experience |
| `subject @ question` | Fill blanks from a Concept or one or more Percepts |
| `subject @~ question` | Ask the same question, also composing parts and generalizing from similar cases |
| `{target} @+= {evidence}` | Add the evidence of matching rows from another linked answer |
| `{target} @-= {evidence}` | Subtract the evidence of matching rows from another linked answer |
| `&operand` | Return the shared answer shape |
| `$operand` | Read Percepts without changing their answer |
| `^operand` | Choose and update every linked output |
| `^~operand` | Draw one result by probability and update every linked output |
| `${*}` | Inspect the ordinary Concepts currently live in the engine |

At the interactive CLI prompt and in the pangine.com workbench, `inspect operand` lists each linked possibility from most to least probable, with its evidence count, probability, and complete-row count, the sources behind that evidence with their signed weights and any composed or generalized rows marked, and all current top ties. `seed n` restarts the generator that `^~` draws from, `partitions n` and `drop partition k` divide remembered experience and lose a partition, and `help` prints the console reference. They are console commands, not `.pae` syntax.

See [pangine.com/grammar.html](https://pangine.com/grammar.html) for the compact reference, [pangine.com/examples.html](https://pangine.com/examples.html) for literal console transcripts, and [pangine.com/demos.html](https://pangine.com/demos.html) for graded questions you can change in the browser.

## Current scope

The Rust prototype includes the parser, canonical Concept graph, mutable Percepts, a read-only global view, remembered experience that can be divided among partition engines, structural questions, correlated answer rows, visible shared answers, immutable Rust Answer values, collapse, grouped input updates, a console that can run commands interactively or from a file, and a browser-local WebAssembly workbench.

Questions preserve complete rows and their source contributions. In Rust, `complete` and `complete_graded` return the rows `@` and `@~` produce, and the Answer API can branch, choose, sample, adjust, and inspect those answers, while the console exposes answer adjustment through `@+=` and `@-=` and source inspection through `inspect`.

Relevance is a signed evidence count read as probabilities. `^` chooses the most probable value, and `^~` draws one by probability from a seeded generator. Persistence, partitions on other machines, automatic callbacks, broad language bindings, and a general LLM adapter are not implemented.

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

The route program re-asks three complete routes, filters recorded episodes by the current result values, adjusts the linked answer from those episode sources, and chooses the complete route. The settings program keeps three outputs linked and chooses them together. An application can replace the input assignments and read the selected outputs without ranking the choices itself. These are capability examples, not fixed application areas.

Run the normal suite with:

```sh
cargo test --workspace --all-targets --release
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

## Contributing

Reproducible bug reports and focused design discussion are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md).

## Licensing

Pangine is source-available under the [PolyForm Noncommercial License 1.0.0](LICENSE.md). Noncommercial use, modification, and distribution are permitted under its terms; commercial use requires separate permission from APU Software, LLC.

This is not an OSI-approved open-source license. See [NOTICE](NOTICE) for ownership and attribution.

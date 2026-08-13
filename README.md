# Pangine

Pangine is an experimental language for writing information as simple shapes and asking questions of those shapes.

Created by [Aaron (`caustik`)](https://github.com/caustik) and released by APU Software, LLC.

I started Pangine from one intuition: the information, the question, and the answer should be made from the same thing. In Pangine, that thing is a **Concept**.

You do not need a machine-learning background to use the current prototype. Pangine is not a trained model, and it does not secretly know what names mean. It only works with the structure and experience you give it.

## Pangine in one minute

Start the console, remember two statements, and ask a question:

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

Here is what happened:

- `[cat]` is a named Concept. Pangine does not assign a built-in meaning to `cat`.
- `[cat]->[purrs]` is an ordered Concept. It can represent a relationship, a sequence, or anything else the application decides.
- `['memory']` is a **Percept**, Pangine's name for a mutable reference. `~=` remembers each complete statement under it.
- `@` asks a question. The quoted Percepts inside the question, `['animal']` and `['sound']`, are the blanks to fill.
- The immediate answer is two complete Concepts. `$['animal']` and `$['sound']` let us inspect one part of those answers afterward.

The complete rows are important. Looking only at the two output columns would no longer tell us that `cat` went with `purrs` and `dog` went with `barks`. The value returned by `@` keeps those pairings together.

## One grammar for information, questions, and answers

A Concept can be a name, an ordered relationship, or an unordered collection of other Concepts:

```text
[cat]
[cat]->[purrs]
[cat][dog]
([person]->[Alice])([pet]->[cat])
```

A question is another Concept with explicit blanks in it:

```text
['memory'] @ ['animal']->['sound']
```

Once the blanks are filled, the answer is an ordinary grounded Concept again. It can be assigned, formatted, parsed, or used as the subject of another question:

```text
command> (['memory'] @ ['animal']->['sound']) @ ['next-animal']->['next-sound']
  {[cat]->[purrs]}
  {[dog]->[barks]}
command> $['next-sound']
  [barks]
  [purrs]
```

A Percept is the mutable handle in this model. It points at remembered Concepts, but it is not a special answer-row type. While a question is running, the Rust engine also carries temporary bookkeeping about which source and which represented occurrence supplied each match. That bookkeeping prevents false combinations, but it is not silently added to the grounded Concept returned by `@`.

## Ask for a chain explicitly

Several relationships can be placed in one question. Reusing the same Percept connects their blanks:

```text
command> ['knowledge'] ~= [Socrates]->[is-a]->[human]
  {[Socrates]->[is-a]->[human]}
command> ['knowledge'] ~= [human]->[is-a]->[mortal]
  {[Socrates]->[is-a]->[human]}
  {[human]->[is-a]->[mortal]}
command> ['knowledge'] @ ([Socrates]->[is-a]->['kind'])(['kind']->[is-a]->['conclusion'])
  {[Socrates]->[is-a]->[human]}
  {[human]->[is-a]->[mortal]}
command> $['conclusion']
  [mortal]
```

Pangine does not know that `is-a` has a logical meaning. The question itself asks for two relationships whose middle Concept must agree. This is how the existing grammar can express a syllogism-shaped use case without adding a special syllogism command.

## Keep represented occurrences together

The current matcher can follow overlapping pieces of a longer ordered Concept without splicing unrelated occurrences together:

```text
command> [Alice]->[gave]->[book]->[to]->[Bob]->[gap]->[Carol]->[gave]->[book]->[to]->[Dave] @ (['giver']->[gave]->['thing'])(['thing']->[to]->['receiver'])
  {[Alice]->[gave]->[book]}{[book]->[to]->[Bob]}
  {[Carol]->[gave]->[book]}{[book]->[to]->[Dave]}
```

The Concept `[book]` appears in both parts of the sequence. Pangine still keeps track of which occurrence supplied each half of a completed answer. It returns Alice with Bob and Carol with Dave. It does not invent Alice with Dave or Carol with Bob.

## Parentheses preserve a complete choice

Parentheses make a complete expression one member of the surrounding unordered Concept. That lets two graph-shaped alternatives remain distinct:

```text
command> (([person]->[Alice])([pet]->[cat]))(([person]->[Bob])([pet]->[dog])) @ ([person]->['who'])([pet]->['animal'])
  {[person]->[Alice]}{[pet]->[cat]}
  {[person]->[Bob]}{[pet]->[dog]}
```

The two parenthesized groups keep each whole group together. Pangine does not mix Alice with dog or Bob with cat. If the direct members are meant to form one shared pool, `*` explicitly merges them:

```text
([A][B])*([C][D])
```

This distinction belongs to the Concept itself. Canonical output preserves the boundary and can be parsed back into the same Concept.

## Repeated experience and the current choice placeholder

Repeating one experience raises its current integer weight:

```text
command> ['world'] ~= [morning]->[birds]
  {[morning]->[birds]}
command> ['world'] ~= [morning]->[birds]
  x2 {[morning]->[birds]}
command> ['world'] ~= [morning]->[traffic]
  x2 {[morning]->[birds]}
  {[morning]->[traffic]}
command> ['world'] @ [morning]->['answer']
  {[morning]->[birds]}
  {[morning]->[traffic]}
command> $['answer']
  x2 [birds]
  [traffic]
command> ^['answer']
  [birds]
```

`x2` is the compact form of two equal unordered members. An implied `x1` is not printed. The current prototype also uses these signed integers to expose how strongly retained experience supported an output.

I think of `@` as leaving the possible answers together and `^` as the point where that state is collapsed to one answer. The present `^` rule is only a deterministic placeholder: choose the greatest positive weight, then use canonical order to break a tie. It is not sampling from logits, a probability calculation, or a finished theory of relevance.

## Capture current input values as experience

The console, pangine.com, and Rust can all provide current values through Percepts. Assign the values with `=`, then mention those Percepts in an experience:

```text
command> ['context-input'] = [opal]
  [opal]
command> ['reading-input'] = [cedar]
  [cedar]
command> ['observations'] ~= [observation]->[context]->['context-input']->[reading]->['reading-input']
  {[observation]->[context]->[opal]->[reading]->[cedar]}
command> ['reading-input'] = [violet]
  [violet]
command> ['observations'] ~= [observation]->[context]->['context-input']->[reading]->['reading-input']
  {[observation]->[context]->[opal]->[reading]->[cedar]}
  {[observation]->[context]->[opal]->[reading]->[violet]}
```

When `~=` runs, Pangine captures assigned Percepts as they are at that moment. Later changes do not rewrite earlier experience. If one assigned input is empty, Pangine does not remember a partial observation.

A Percept populated through `~=` remains a reference when another experience mentions it. Use `$` when you want to follow all Percepts in an expression. This is how the current implementation works. It does not create separate Percept types.

Rust callers can validate and set a complete input group with `set_percept_values`, remember a Percept-bearing template with `perform_experience`, and read output Percepts after Pangine questions and chooses. `evaluate_concept` is available when explicit grounding is needed without remembering the result. The caller controls when a complete update occurs, but Pangine retains the experience, forms the candidate state, and makes the represented choice.

## Grammar summary

| Form | Plain-language meaning |
| --- | --- |
| `[]` | No Concept |
| `[name]` | A named Concept |
| `['memory']` | A mutable Percept reference |
| `[A]->[B]->[C]` | One ordered Concept |
| `[A][B]` | One unordered Concept containing `A` and `B` |
| `(expression)` | Keep the complete expression as one surrounding member |
| `[A]*[B]` | Merge the direct unordered members |
| `[A]/[B]` | Merge with an inverted right side |
| `![A]` | An inverted member |
| `x2[A]` | Two copies of the next complete member; `x1` is omitted |
| `['memory'] = expression` | Replace a Percept's value |
| `['memory'] ~= expression` | Capture assigned inputs, then remember one complete expression |
| `subject @ question` | Fill the question's Percept blanks from the subject |
| `$operand` | Evaluate the Percepts inside an expression |
| `^['choice']` | Use the current deterministic choice placeholder |
| `$['*']` | Inspect the ordinary Concepts currently live in the engine |

See [pangine.com/grammar.html](https://pangine.com/grammar.html) for the complete compact reference and [pangine.com/examples.html](https://pangine.com/examples.html) for more literal console transcripts.

## Current implementation

The Rust prototype currently includes:

- A parser and canonical formatter
- Canonical Concept graphs and mutable Percepts
- Complete remembered statements with signed integer weights
- Questions over one Concept, one Percept, or several selected Percepts
- Questions made from one relationship or several relationships joined through shared blanks
- Complete answer rows that remain ordinary Concepts and can be questioned again
- Matching inside unordered groups and contiguous ordered paths without unsupported cross-pairing
- Grouped Rust input updates, experience capture, explicit evaluation, and readable output Percepts
- A deterministic placeholder behind `^`
- A command-line console, a browser-local WebAssembly workbench, tests, and research probes

## Open work

Pangine began as an intuition about a Bayesian semantic hypergraph. I still care about keeping uncertainty visible and allowing accumulated experience to shape a later choice. More matching experience can matter, much as more active signals can matter in a body, without declaring every integer weight to be probability, confidence, or truth.

The application provides observations, current values, timing, and represented context, so that experience shapes the answer. I still want the Pangine program to form the candidates and choose among them instead of hiding that work in the application. I do not want to make this a strict boundary while the integration is still being worked out.

A real Rust caller is the next place to exercise the input-to-experience-to-output loop. An LLM could eventually participate as an identified source of experience or as a consumer of an output Percept. It should not silently become Pangine's question selector, relevance calculator, or final judge.

A learned decision step, a logit sampler, probabilities, persistence, automatic application callbacks, and a distributed runtime are not implemented yet. The ignored research fixtures contain the detailed experiments and open questions.

## Run Pangine

Install a current stable Rust toolchain, then:

```sh
git clone https://github.com/caustik/pangine.git
cd pangine
cargo run --bin pangine-console
```

Run the normal verification suite with:

```sh
cargo test --workspace --all-targets --release
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

The repository also contains ignored research tests. They record provisional questions and boundaries; they are not compatibility promises.

## Contributing

Reproducible bug reports and focused design discussion are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md) for the current contribution policy.

## Licensing

Pangine is source-available under the [PolyForm Noncommercial License 1.0.0](LICENSE.md). Noncommercial use, modification, and distribution are permitted under its terms; commercial use requires separate permission from APU Software, LLC.

This is not an OSI-approved open-source license. See [NOTICE](NOTICE) for ownership and attribution.

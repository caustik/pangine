# Pangine

Pangine is an experimental semantic state and reasoning engine written in Rust.

I originally came up with Pangine by writing down pieces of information in a semantic shape, asking questions about them, and then reasoning backward from what the grammar should imply. Pangine explores whether experience, retained state, and questions can all use that same small grammar.

Concepts have canonical forms and can be composed without giving their names a built-in ontology. Experiencing a concept recursively exposes everything it contains, at every level. A question uses literal matches and implied wildcard possibilities to produce weighted answers. The hope is that several partial matches can converge on an answer even when no literal fact was stored.

The larger question is whether this can become an inference system in its own right. Pangine's weighted possibilities could be sampled directly, used to guide an LLM, or combined with other systems. It may end up supplementing current LLM technology, or it may develop into an alternative approach to inference. It is too early to know.

Pangine does not assume that a language model has to perform the inference. If an LLM is involved, it could translate information into and out of Pangine while Pangine retains inspectable state and performs its own reasoning.

Created by [Aaron (`caustik`)](https://github.com/caustik) and released by APU Software, LLC.

## The core idea

A Pangine expression describes a semantic shape. Named concepts can be composed into unions, directed correlations, observations, and deeper structures. Applications decide what those concepts mean.

```text
{[cat]->[purrs]}
{{[cat]->[purrs]}->{[sound]->[soft]}}
```

A correlation describes a relationship in the observed material. An observation can preserve who or what supplied an entire Concept without changing the Concept it contains:

```text
{[rain]->[wet_ground]}
?[weather_station]:{[rain]->[wet_ground]}
```

The second expression reads as "the weather station observed or asserted that rain relates to wet ground." Both sides may be any recursive Concept. The observer may be a person, sensor, document, model run, or another observation. When an expression is experienced without an explicit observer, it is globally scoped. The grammar does not yet assign observer-specific confidence, replay, or retraction behavior.

A named percept holds state. Assignment stores a value, experience accumulates what has been observed, a question binds output percepts, and evaluation materializes their current values.

```text
['memory'] ~= {[cat]->[purrs]}
['memory'] @ {['animal']->[purrs]}
$['animal']
```

The first statement experiences a correlation. The second asks which experienced concepts can occupy the left side of that shape. The third evaluates the resulting ranked candidates.

Every concept inside an experience is also experienced. Nothing privileges the root or any other level. In theory, this implies every recursive combination of literal and wildcard structure. In practice, that closure is too large to store, so questions fold the compatible projections together when they are needed. These abstraction levels are somewhat like hidden features in a neural network, except they come from the grammar and remain possible to inspect.

The hard part is relevance. Direct evidence, many independent hints at different levels of abstraction, and a generic wildcard possibility should not all have the same weight. The current projection weights are deterministic, but they are not calibrated probabilities and they are not the final answer.

## Scaling direction

When I first thought about scaling Pangine, the model was closer to map/reduce than GPU acceleration. Canonical form gives concepts a stable identity, but no concept needs a permanent machine owner. Distributed entities could each hold a roughly even subset of relevance. A percept operation could unfold across those entities, perform the local work, and then reduce the partial candidate weights.

Changing how relevance is divided should not change the answer. If an entity goes down, Pangine should keep working with less relevance instead of breaking the model. With enough entities and reasonably balanced relevance, that failure becomes a relatively small change in the available evidence. Rare or important evidence may still need replication.

I have now tested one small part of this idea. In a test-only experiment, I split three source-labeled observations into different groups, folded each group separately, and combined the resulting support Concepts. Every grouping produced the same canonical result as folding all three observations together. This happened inside one Pangine engine, so it is not distributed execution yet. Sending the same partial result twice still counts it twice, which leaves observation identity and duplicate delivery as open questions.

The recursive closure still has to remain implied. Sending every wildcard permutation over the network would replace a storage problem with a messaging problem. Distributed execution is only a design direction today. GPU acceleration could still be useful inside any of the workers.

## Current status

The current Rust implementation includes:

- Parsing and canonical formatting of the grammar
- Weakly interned, canonical concept graphs
- Engine-owned percept state and relevance operations
- Recursive experience accumulation
- Lazy recursive wildcard projection and ranked question bindings
- An interactive console, a Rust example, and compatibility tests

The relevance model, decision and sampling semantics, persistence, model integration, and language bindings are still open work. Pangine does not currently include an LLM, vector database, llama.cpp integration, or Python package.

## Quick start

Install a current stable Rust toolchain, then clone and test the project:

```sh
git clone https://github.com/caustik/pangine.git
cd pangine
cargo test --all-targets --release
```

Run the induction example:

```sh
cargo run --example induction
```

```text
hypothesis: repeated partial experience can outweigh one complete observation
complete experience: {[C]->[A]}*{[B]->[D]}
partial experience:  {[E]->[A]}*{[P1]->[Q1]}
partial experience:  {[E]->[A]}*{[P2]->[Q2]}
partial experience:  {[E]->[A]}*{[P3]->[Q3]}
question:            {['X']->[A]}*{[B]->[D]}
ranked candidates:   <x18[E], x12[C], x3[B], x3[P1], x3[P2], x3[P3]>
selected candidate:  [E]
result: E wins without the complete E-shaped observation ever being experienced
limitation: these strengths are deterministic projection scores, not calibrated probabilities
```

The complete observation supports C under the entire question shape. E only appears in three partial observations, each paired with a different distractor. The question combines those partial structural matches and ranks E above C even though the complete E-shaped observation was never stored. This is a bounded induction result, not a claim that the current relevance strengths are calibrated probabilities.

The smaller `semantic_memory` example demonstrates assignment, canonical storage, and recall:

```sh
cargo run --example semantic_memory
```

## Console

Start the interactive console with:

```sh
cargo run --bin pangine-console
```

Enter `help` for the complete syntax summary and `quit` to exit.

```text
command> [cat]->[purrs]
  {[cat]->[purrs]}
```

## Language at a glance

| Form | Meaning |
| --- | --- |
| `[]` | Null or no concept |
| `[name]` | Named concept |
| `['memory']` | Named percept reference |
| `[A][B]` | Union |
| `[A]*[B]` | Flattening merge |
| `[A]/[B]` | Merge with an inverted right operand |
| `![A]` | Inversion |
| `[A]->[B]` | Directed correlation |
| `?[observer]:[observation]` | Observation made or asserted by an observer |
| `<50%x2[A]>` | Probability and strength relevance |
| `['memory'] = expression` | Assign percept state |
| `['memory'] ~= expression` | Accumulate an experience |
| `['memory'] @ expression` | Bind outputs and return the unresolved question shape |
| `$operand` | Recursively evaluate every percept in its operand |

Statements may be separated with semicolons. C-style block comments and C++-style line comments are ignored. Canonical output may differ from accepted input syntax; for example, `[A]->[B]` formats as `{[A]->[B]}`.

## Engine model

`ConceptId` is a cloneable owning handle. Named and composite concepts are weakly interned, so a live concept has canonical identity without forcing every concept to remain allocated for the lifetime of the engine. Named percepts are engine-owned state roots, while their values are kept outside concept nodes to avoid reference cycles.

Parsing distinguishes a valid null result from malformed input and I/O failure. Parsing is deliberately non-transactional: successful percept mutations before a later error remain applied.

Experience stores exact recursively unrolled structure and accumulated relevance. Questions lazily fold the implied recursive wildcard projections into distinct ranked output bindings, so the combinatorial closure does not have to be stored. Because `@` binds more tightly than `$`, `$['memory'] @ expression` resolves the returned question shape only when an evaluated snapshot is explicitly requested.

The crate has no third-party runtime dependencies and forbids unsafe Rust.

## Project layout

```text
src/                 Rust library and interactive console
examples/            Small runnable demonstrations
tests/               Behavioral, compatibility, and lifetime tests
tests/research/      Reproducible experiments that are not accepted semantics
tests/fixtures/      Grammar scripts used by the compatibility suite
```

## Contributing

Reproducible bug reports and focused design discussion are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md) for the current contribution policy.

## Licensing

Pangine is source-available under the [PolyForm Noncommercial License 1.0.0](LICENSE.md). Noncommercial use, modification, and distribution are permitted under its terms; commercial use requires separate permission from APU Software, LLC.

This is not an OSI-approved open-source license. See [NOTICE](NOTICE) for ownership and attribution.

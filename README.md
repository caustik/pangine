# Pangine

Pangine is a deterministic compositional grammar and semantic state engine written in Rust. It parses compact symbolic expressions, interns them into canonical concept graphs, and stores structured state in named percepts without imposing a fixed ontology or model-specific interpretation.

Created by [Aaron (`caustik`)](https://github.com/caustik) and released by APU Software, LLC.

Pangine is experimental. The parser, canonical representation, state ownership, relevance operations, console, and compatibility tests are implemented. Pangine is not an LLM and does not currently include model inference, persistence, retrieval infrastructure, or Python bindings.

## Quick start

Install a current stable Rust toolchain, then clone and test the project:

```sh
git clone https://github.com/caustik/pangine.git
cd pangine
cargo test --all-targets --release
```

Run the semantic-memory example:

```sh
cargo run --example semantic_memory
```

```text
stored:   {[cat]->[purrs]}
recalled: {[cat]->[purrs]}
```

The example stores a directed relation in a named percept, recalls it through the grammar, and verifies that both paths resolve to the same canonical concept.

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

## Language surface

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
| `?[A]:[B]` | Dependency pair |
| `<50%x2[A]>` | Probability and strength relevance |
| `['memory'] = expression` | Assign percept state |
| `['memory'] ~= expression` | Accumulate an experience |
| `['memory'] @ expression` | Bind outputs and return the unresolved question shape |
| `$operand` | Recursively evaluate every percept in its operand |

Statements may be separated with semicolons. C-style block comments and C++-style line comments are ignored. Canonical output may differ from accepted input syntax; for example, `[A]->[B]` formats as `{[A]->[B]}`.

## Engine model

`ConceptId` is a cloneable owning handle. Named and composite concepts are weakly interned, so a live concept has canonical identity without forcing every concept to remain allocated for the lifetime of the engine. Named percepts are engine-owned state roots, while their values are kept outside concept nodes to avoid reference cycles.

Parsing distinguishes a valid null result from malformed input and I/O failure. Parsing is deliberately non-transactional: successful percept mutations before a later error remain applied.

Experience stores exact recursively unrolled structure and accumulated relevance. Questions lazily fold the implied recursive wildcard projections into distinct ranked output bindings and cheaply return their unresolved question shape; no combinatorial wildcard closure is materialized. Because `@` binds more tightly than `$`, `$['memory'] @ expression` recursively resolves that returned shape only when an evaluated result is explicitly requested.

## Project layout

```text
src/                 Rust library and interactive console
examples/            Small runnable demonstrations
tests/               Behavioral, compatibility, and lifetime tests
tests/fixtures/      Grammar scripts used by the compatibility suite
```

The crate has no third-party runtime dependencies and forbids unsafe Rust.

## Licensing

Pangine is source-available under the [PolyForm Noncommercial License 1.0.0](LICENSE.md). Noncommercial use, modification, and distribution are permitted under its terms; commercial use requires separate permission from APU Software, LLC.

This is not an OSI-approved open-source license. See [NOTICE](NOTICE) for ownership and attribution.

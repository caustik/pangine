# Pangine

Pangine is an experimental semantic state and reasoning language and engine.

Created by [Aaron (`caustik`)](https://github.com/caustik) and released by APU Software, LLC.

<a id="tutorial"></a>

## Learning Pangine from `[]`

I find Pangine easiest to understand by beginning with literally nothing and adding one idea at a time.

Install a current stable Rust toolchain, then start a fresh console:

```sh
git clone https://github.com/caustik/pangine.git
cd pangine
cargo run --bin pangine-console
```

Everything below is an actual `pangine-console` transcript. Text after `command>` is what I typed. The indented lines are exactly what the console printed.

Pangine does not know what names such as `cat`, `Alice`, or `full-test` mean. They are ordinary names chosen for the examples. Their meaning comes from the person or program using the language.

### Start with nothing

```text
command> []
  []
command> [cat]
  [cat]
```

`[]` is the null Concept. It means there is no Concept here. `[cat]` is a named Concept, and Pangine treats its name as opaque.

### Put Concepts together

Writing Concepts next to each other makes an unordered composition, which the grammar calls a union:

```text
command> [cat][dog]
  [cat]
  [dog]
command> [cat][cat][dog]
  x2 [cat]
  [dog]
```

Repeating the same unordered member increases its retained strength. Canonical output shortens two copies to `x2`.

Parentheses group an expression but do not create another kind of Concept. Adjacency can retain a complete group as one operand, while `*` flattens before merging:

```text
command> ([A][B])([A][B])
  x2 [A][B]
command> ([A][B])*([A][B])
  x2 [A]
  x2 [B]
```

`/` is the inverse form of the flattening merge:

```text
command> ([A][B])/([A][C])
  [B]
  ![C]
command> [cat]*![cat]
  []
```

The positive and inverted copies of `A` cancel. Inversion is part of Concept composition. Pangine does not automatically interpret it as natural-language negation.

### Attach relevance

Relevance coefficients prefix the next union operand:

```text
command> 50%x2[cat]x3[dog]
  50%x2 [cat]
  x3 [dog]
command> x2([cat][dog])
  x2 [cat][dog]
```

`50%` is the probability component and `x2` is the strength component attached to `cat`. `x3` applies separately to `dog`. Parentheses let one coefficient apply to a complete group.

These values currently participate in deterministic composition and selection. Question answers use the experience-backed support rule explained below instead of treating these coefficients as evidence. They are not calibrated probabilities or confidence claims, and the richer relevance model is still open work.

### Put Concepts in order

`->` makes one ordered composition:

```text
command> [cat]->[purrs]
  {[cat]->[purrs]}
command> [cat]->[purrs]->[soft]
  {[cat]->[purrs]->[soft]}
command> ([cat]->[purrs])->[soft]
  {{[cat]->[purrs]}->[soft]}
```

The console adds braces because `{[cat]->[purrs]}` is the canonical form. Canonical output is valid Pangine that parses back into the same Concept.

The three-part chain is flat: it retains `cat`, `purrs`, and `soft` as three ordered occurrences. It is not secretly grouped as either `(cat -> purrs) -> soft` or `cat -> (purrs -> soft)`. If I add parentheses explicitly, as in the last command, the inner ordered composition becomes one component of the outer one. Unlike unordered composition, repeating a component in an ordered composition keeps a separate position.

### Give a Percept mutable state

A quoted name refers to a Percept:

```text
command> ['memory']
  ['memory']
command> $['memory']
  []
command> ['memory'] = {[cat]->[purrs]}
  {[cat]->[purrs]}
command> ['memory']
  ['memory']
command> $['memory']
  {[cat]->[purrs]}
```

The reference remains `['memory']`. `$` evaluates its current value. Before assignment that value is null.

The ordinary mutation operators work on the current materialized value and replace the Percept with one resulting root:

```text
command> ['state'] = [A]
  [A]
command> ['state'] += [B]
  [A]
  [B]
command> ['state'] -= [A]
  [B]
command> ['state'] *= [C]
  [B]
  [C]
command> ['state'] /= [C]
  [B]
```

### Turn a Percept into a source of experience

`~=` adds one complete Concept as an exact root owned by the Percept:

```text
command> ['Alice'] ~= {[cat]->[purrs]}
  {[cat]->[purrs]}
command> ['Bob'] ~= {[cat]->[meows]}
  {[cat]->[meows]}
```

Each `~=` command is one experience. The complete input is its exact root, so I do not need to invent an event name just to preserve that boundary. Repeating an equal root increments its occurrence count. The root stays an ordinary Concept. Pangine does not put it or the Percept into a special experience mode.

This matters because the complete boundary remains available even though a question can inspect the recursive pieces inside it. Pangine derives those pieces when it asks a question. It does not store a second expanded copy of the experience.

`$['Alice']` is a convenient combined view:

```text
command> $['Alice']
  {[cat]->[purrs]}
```

That combined view is not always a lossless account of the roots. One root `[A][B]` and two roots `[A]`, `[B]` can materialize to the same output. The engine API therefore exposes the unique exact roots with `get_percept_roots` and the occurrence count of each root with `get_percept_root_count`.

### Ask one source

`@` asks a Percept and binds the Percepts inside the question:

```text
command> ['Alice'] @ {[cat]->['sound']}
  {[cat]->['sound']}
command> $['sound']
  [purrs]
```

The immediate result is still the unresolved question shape. `$['sound']` shows the answer candidates that were written into the output Percept.

The fixed `[cat]` must match exactly. `['sound']` is the wildcard position Pangine fills. Ordinary Concepts in a question are never implicit weak wildcards.

A longer ordered root can supply an exact contiguous path without being stored a second time:

```text
command> ['Sequence'] ~= [cat]->[eats]->[cat_food]
  {[cat]->[eats]->[cat_food]}
command> ['Sequence'] @ ['what']->[eats]
  {['what']->[eats]}
command> $['what']
  [cat]
```

`cat -> eats` is present as a contiguous path, so `what` becomes `cat`. `eats` is not also returned: that would require the nonexistent path `eats -> eats`. The complete three-part root remains available as context for later relevance work.

### Let represented context widen an answer

A fresh console keeps this example separate from the earlier Alice and Bob state:

```text
command> ['Room'] ~= [kitchen]->[connected-to]->[living-room]
  {[kitchen]->[connected-to]->[living-room]}
command> ['Room'] ~= [kitchen]->[sound]->[fridge-hum]
  {[kitchen]->[connected-to]->[living-room]}
  {[kitchen]->[sound]->[fridge-hum]}
command> ['Room'] ~= [living-room]->[sound]->[music]
  {[kitchen]->[connected-to]->[living-room]}
  {[kitchen]->[sound]->[fridge-hum]}
  {[living-room]->[sound]->[music]}
command> ['Room'] @ [kitchen]->[sound]->['answer']
  {[kitchen]->[sound]->['answer']}
command> $['answer']
  [fridge-hum]
  [music]
```

`fridge-hum` is the direct answer. `music` is an additional candidate because the selected experience contains a represented path from `kitchen` to `living-room`, and `living-room -> sound -> music` otherwise matches the question exactly.

The starting `kitchen` Concept is the only part that may follow that context. The later fixed `sound` Concept must still match exactly. A possible answer cannot use its own sound relationship to create the route that makes itself possible, and only roots selected on the left of `@` participate.

This does not mean Pangine has decided that `music` is the right answer. It means Pangine can retain a potentially relevant indirect answer instead of discarding it. Directness, shorter routes, more routes, and more matching surroundings do not add support by themselves. Both answers have one supporting experience root here, so `^` still uses its deterministic tie rule.

### Let experience counts change the choice

Every `~=` command already represents one experience. If the same thing is experienced twice, I can simply say it twice:

```text
command> ['world'] ~= [morning]*[birds]
  [morning]
  [birds]
command> ['world'] ~= [morning]*[birds]
  x2 [birds][morning]
command> ['world'] ~= [morning]*[traffic]
  x2 [birds][morning]
  [morning][traffic]
command> ['world'] @ [morning]*['answer']
  [morning]
  ['answer']
command> $['answer']
  x2 [birds]
  [traffic]
command> ^['answer']
  [birds]
```

The complete root is the implicit experience boundary. The first two commands therefore give `birds` two units of support, while the third gives `traffic` one. I do not need to add `event-1` and `event-2` Concepts unless those event identities are meaningful to the information itself.

The selected Percept and exact root stay attached to each match. Repeating one exact root increases its stored count. Two unequal roots under the same Percept are two experiences, and equal roots under `Alice` and `Bob` are also separate source contributions. Finding the same answer several times inside one exact root, including through its recursive pieces or context routes, still uses that root only once.

`x2` is a count of supporting experience occurrences. It is useful to the current `^` choice, but it is not a claim that birds are twice as true or have a two-thirds real-world probability. A richer interpretation still needs explicit input describing reliability, dependence, and counterevidence.

### Ask several sources together

Writing several plain Percepts on the left of `@` selects all of them:

```text
command> ['Alice']['Bob'] @ {[cat]->['shared-sound']}
  {[cat]->['shared-sound']}
command> $['shared-sound']
  [purrs]
  [meows]
command> ^['shared-sound']
  [meows]
```

`@` applies after the complete expression on its left. Parentheses are optional, so this is the normal form:

```text
['Alice']['Bob'] @ {[cat]->['sound']}
```

`(['Alice']['Bob']) @ ...` means the same thing, but the parentheses are not needed.

The selected Percepts are the sources. Experiencing an equal root twice under `Alice` contributes twice. Experiencing it once under both `Alice` and `Bob` also contributes twice.

`^` currently selects the greatest positive weight. An exact tie uses the earliest canonical Concept spelling, which is why `meows` wins the tie above. This is a stable fallback, not a claim that `meows` is inherently more likely.

### Reuse one output identity

The same output Percept keeps one identity throughout a question:

```text
command> ['reviewer'] ~= {[review]->[review]}
  {[review]->[review]}
command> ['shipper'] ~= {[prepare]->[ship]}
  {[prepare]->[ship]}
command> ['reviewer']['shipper'] @ {['same']->['same']}
  {['same']->['same']}
command> ^['same']
  [review]
```

`{['same']->['same']}` asks for one Concept that can occupy both positions. `{['left']->['right']}` would allow independent answers.

### Use scripts and comments

Statements may be separated with semicolons. The parser also accepts C++-style line comments and C-style block comments:

```text
command> ['steps'] = [draft]; ['steps'] += [review]; $['steps']
  [draft]
  [review]
command> [cat] // the rest of this line is a comment
  [cat]
command> /* comments can also sit inside a line */ [dog]
  [dog]
```

### Inspect the live ordinary Concepts

`$['*']` is the global inspection view:

A fresh console keeps this inspection example small:

```text
command> ['one'] = [A]
  [A]
command> ['two'] = {[A]->[B]}
  {[A]->[B]}
command> $['*']
  [A]
  [B]
  {[A]->[B]}
```

It returns every ordinary Concept currently retained by the engine. It is read-only and computed when requested. It is useful for inspecting a session, but it is not persistence or a serialized database.

### A deeper example: choosing a build route

This example starts with a fresh console. The Percept itself identifies the source:

```text
command> ['Maintainer'] ~= {[full-test]->[cli-runner]}
  {[full-test]->[cli-runner]}
command> ['Maintainer'] ~= {[lint]->[clippy]}
  {[full-test]->[cli-runner]}
  {[lint]->[clippy]}
command> ['Legacy-note'] ~= {[full-test]->[cargo]}
  {[full-test]->[cargo]}
```

Asking both sources retains the two exact answers. The unrelated lint root does not match the fixed `full-test` Concept:

```text
command> ['Maintainer']['Legacy-note'] @ {[full-test]->['route']}
  {[full-test]->['route']}
command> $['route']
  [cli-runner]
  [cargo]
command> ^['route']
  [cargo]
```

Pangine does not infer that the maintainer is more authoritative, that the legacy note is older, or that either route should overwrite the other. `^` only applies the current deterministic tie rule.

The caller can instead choose which source to ask:

```text
command> ['Maintainer'] @ {[full-test]->['maintainer-route']}
  {[full-test]->['maintainer-route']}
command> $['maintainer-route']
  [cli-runner]
command> ^['maintainer-route']
  [cli-runner]
```

Pangine answers from the selected Percept roots. The source names remain opaque, and source selection is explicit in the question.

### What this walkthrough does not settle

This walkthrough covers the current console surface: canonical composition, relevance, Percept state, exact-root experience, direct and contextual questions, one-source and multi-source selection, shared output identity, decision, scripts, and inspection.

It does not establish calibrated probabilities, automatic conflict resolution, persistence, authorization, a numeric or temporal domain grammar, richer sampler behavior, or a production revision API.

## Why I built Pangine

I originally came up with Pangine by writing down pieces of information in a semantic shape, asking questions about them, and then reasoning backward from what the grammar should imply. Pangine explores whether experience, retained state, and questions can all use that same small grammar.

Concepts have canonical forms and can be composed without giving their names a built-in ontology. A Percept retains exact complete roots. A question can use both those roots and the recursive structure inside them without turning experience into a separate kind of Concept.

The larger question is whether this can become an inference system in its own right. The deterministic `^` rule gives Pangine a stable baseline for choosing among weighted possibilities. A future sampler could provide richer candidate selection, but that does not require an LLM to generate or translate Pangine.

## Scaling direction

When I first thought about scaling Pangine, the model was closer to map/reduce than a central database. Canonical form gives Concepts a stable identity, but no Concept needs a permanent machine owner. Each partition can retain flat exact root edges from Percepts to canonical Concepts.

Those exact roots and their occurrence counts are authoritative. A canonical Concept is routed to one member rather than broadcast to every member, so a repeated experience is counted where that Concept lives. A question member can return local candidate totals and reduction can add them. It does not need event IDs or cross-member duplicate tracking. If a partition is lost, Pangine should continue from the roots that remain.

An operating server may keep a densely connected in-memory representation or a disposable lookup to make matching fast. That structure is a cache over the flat canonical roots, not another source of truth. It can be rebuilt, dropped, or scoped to one partition.

Question evaluation snapshots the selected exact roots and their counts before writing any outputs. It derives only the recursive match views needed by the current question and builds disposable in-memory connections from the retained Concept structure. A finite search can then discover an indirect ordered answer without making that temporary structure authoritative or storing it as another kind of experience. Cycles terminate, and several matcher routes through one root do not multiply its count.

This first production implementation rebuilds those connections from the selected roots for each question. It establishes the behavior and gives us a testable correctness path, but it does not yet establish large-scale retrieval performance. Efficient caching, partitioned execution, and the final relevance model remain active research areas.

## Current status

The current implementation is written in Rust. It includes:

- Parsing and canonical formatting of the grammar
- Weakly interned, canonical Concept graphs
- Unified Percept state built from exact complete roots
- Counted experience occurrences over unique exact Percept roots
- One-source and unparenthesized multi-source question selection
- Lazy exact recursive matching with explicit Percept wildcards and shared repeated-output bindings
- Source-backed contextual candidate discovery for ordered questions across nested, ordered, unordered, weighted, negative, cyclic, and multi-source Concept structure
- Exact-root-backed question scoring with recursive and contextual route deduplication
- Deterministic positive-weight greedy choice with canonical tie handling
- `$['*']` global inspection
- An interactive console, focused regression tests, and a browser-local WebAssembly workbench

Context changes which candidates Pangine can find, and supporting experience occurrences can now change which candidate wins. Graph shape alone still does not create a preference. A richer relevance model, decision and sampling semantics, scalable retrieval, persistence, distributed execution, and general-purpose application bindings are still open work. Pangine does not currently include llama.cpp or another external sampler, a vector database, or a Python package.

To run the complete verification suite:

```sh
cargo test --workspace --all-targets --release
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

## Language at a glance

| Form | Meaning |
| --- | --- |
| `[]` | Null or no Concept |
| `[name]` | Named Concept |
| `['memory']` | Named Percept reference |
| `(expression)` | Grouping |
| `[A][B]` | Union |
| `[A]*[B]` | Flattening merge |
| `[A]/[B]` | Merge with an inverted right operand |
| `![A]` | Inversion |
| `[A]->[B]->[C]` | One flat ordered composition; canonical output includes braces |
| `50%x2[A]` | Probability and strength relevance on the next operand |
| `['memory'] = expression` | Replace a Percept with zero or one root |
| `['memory'] += expression` | Add to the materialized value, then retain one result root |
| `['memory'] -= expression` | Subtract from the materialized value, then retain one result root |
| `['memory'] *= expression` | Flattening merge, then retain one result root |
| `['memory'] /= expression` | Inverse merge, then retain one result root |
| `['memory'] ~= expression` | Record one experience of an exact complete root |
| `['memory'] @ expression` | Ask one source and bind output Percepts |
| `['Alice']['Bob'] @ expression` | Ask several sources together; parentheses are optional |
| `$operand` | Recursively evaluate every Percept in the operand |
| `^['choice']` | Select the greatest positive weight, using canonical spelling for exact ties |
| `$['*']` | Inspect all live ordinary Concepts through a read-only computed view |

Statements may be separated with semicolons. C-style block comments and C++-style line comments are ignored. Canonical output may differ from accepted input syntax. For example, `[A]->[B]` formats as `{[A]->[B]}`.

## Contributing

Reproducible bug reports and focused design discussion are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md) for the current contribution policy.

## Licensing

Pangine is source-available under the [PolyForm Noncommercial License 1.0.0](LICENSE.md). Noncommercial use, modification, and distribution are permitted under its terms; commercial use requires separate permission from APU Software, LLC.

This is not an OSI-approved open-source license. See [NOTICE](NOTICE) for ownership and attribution.

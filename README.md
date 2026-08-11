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

This walkthrough describes the current prototype, not a finished language specification. The parser and output are real. The detailed rules used by structural completion, answer totals, and `^` are experiments that I expect to revisit as the larger design becomes clearer.

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

Repeating the same unordered member increases its retained multiplicity. Canonical output shortens two copies to `x2`.

Parentheses make a complete expression one operand of the surrounding composition. A parenthesized unordered Concept therefore remains an ordinary member of another unordered Concept:

```text
command> [A][B][A][B]
  x2 [A]
  x2 [B]
command> ([A][B])([A][B])
  x2([A][B])
command> ([A][B])*([A][B])
  x2 [A]
  x2 [B]
```

The first Concept has four members at one level. The second has two equal members, each of which is the complete Concept `[A][B]`. `*` explicitly merges the direct members of its operands, so the third input reaches the same flat members as the first. This distinction belongs to Concept construction, not only to text parsing. The canonical Concept graph retains the nested member, and its formatted text parses back into the same graph.

A surrounding ordered Concept likewise retains an unordered Concept as one position:

```text
command> ([A][B])->[C]
  {[A][B]->[C]}
```

`/` is the inverse form of the union merge:

```text
command> ([A][B])/([A][C])
  [B]
  ![C]
command> [cat]![cat]
  []
```

The positive and inverted copies of `A` cancel. Inversion is part of Concept composition. Pangine does not automatically interpret it as natural-language negation.

### Write multiplicity explicitly

The `x` prefix is the explicit form of repeated union membership:

```text
command> x2[cat]x3[dog]
  x3 [dog]
  x2 [cat]
command> x2([cat][dog])
  x2([cat][dog])
```

`x2[cat]` is the same Concept as `[cat][cat]`. The prefix applies to the next unordered operand, so `x3` applies separately to `dog`. In `x2([cat][dog])`, the coefficient applies to the complete parenthesized Concept rather than distributing into its members. Inversion is the negative form: `x-1[cat]` formats as `![cat]`. `/` remains the explicit way to merge the inverted members of its right operand. Coefficients are currently signed 64-bit integers; decimal coefficients are not valid syntax, and an out-of-range composition reports an error rather than wrapping.

The current engine retains complete Concepts as direct Percept subconcepts with signed integer relevance. It uses the same `ConceptMap` representation as an ordinary unordered Concept rather than a separate experience-root table. What a later decision should calculate from that accumulated information remains open. The current `^` implementation only supplies the simple deterministic placeholder explained below.

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

The ordinary mutation operators work on the current materialized value and replace the Percept with one resulting subconcept:

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

`~=` adds one complete Concept as a direct subconcept of the Percept:

```text
command> ['Alice'] ~= {[cat]->[purrs]}
  {[cat]->[purrs]}
command> ['Bob'] ~= {[cat]->[meows]}
  {[cat]->[meows]}
```

Each `~=` command is one experience. The complete input becomes one direct subconcept, so I do not need to invent an event name just to preserve that boundary. Repeating an equal Concept adds the default relevance of one to the same member. The member stays an ordinary Concept. Pangine does not put it or the Percept into a special experience mode.

This matters because the complete boundary remains available even though a question can inspect the recursive pieces inside it. Pangine derives those pieces when it asks a question. It does not store a second expanded copy of the experience.

`$['Alice']` is a convenient combined view:

```text
command> $['Alice']
  {[cat]->[purrs]}
```

That combined view is not always a lossless account of the Percept's direct structure. One subconcept `[A][B]` and two subconcepts `[A]`, `[B]` can materialize to the same output. `get_relevance_map(&percept)` exposes the direct members and their `Relevance`, just as it does for an ordinary unordered Concept.

### Ask one source

`@` asks the Concept on its left to complete a structural question. A plain Percept selects its direct retained subconcepts. Percepts inside the question are holes to fill:

```text
command> ['Alice'] @ {[cat]->['sound']}
  {[cat]->[purrs]}
command> $['sound']
  [purrs]
```

The immediate result is the grounded question row. `$['sound']` shows the compatibility view also written into the output Percept. Returning the row matters when a question has several holes: the values remain correlated instead of becoming an accidental cross-product.

In the current matcher, the fixed `[cat]` must match exactly. `['sound']` is the wildcard position Pangine fills. Ordinary Concepts are not treated as implicit weak wildcards.

An ordinary grounded Concept can be the subject directly. Pangine treats the complete value on the left as one source Concept and applies the same matcher:

```text
command> {[cat]->[eats]}{[dog]->[sleeps]} @ ['what']->['whats']
  {[cat]->[eats]}
  {[dog]->[sleeps]}
command> $['what']
  [cat]
  [dog]
command> $['whats']
  [eats]
  [sleeps]
```

This does not turn answer rows into a special kind of Concept. It means any grounded Concept can participate in the same grammar. A direct subject has default relevance and no Percept owner. Selecting a Percept instead preserves which direct subconcept participated, its relevance, and its owner for the Rust completion result.

A longer ordered Concept can supply an exact contiguous path without being stored a second time:

```text
command> ['Sequence'] ~= [cat]->[eats]->[cat_food]
  {[cat]->[eats]->[cat_food]}
command> ['Sequence'] @ ['what']->[eats]
  {[cat]->[eats]}
command> $['what']
  [cat]
```

`cat -> eats` is present as a contiguous path, so `what` becomes `cat`. `eats` is not also returned: that would require the nonexistent path `eats -> eats`. The complete three-part Concept remains available as context for later questions.

### Ask for composition explicitly

The next commands add a separate source without changing the earlier Alice and Bob members:

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
  {[kitchen]->[sound]->[fridge-hum]}
command> $['answer']
  [fridge-hum]
```

One relation atom asks for one relation, so this returns only the direct answer. If I want Pangine to compose the connection and sound relationships, I write both atoms and share a Percept between them:

```text
command> ['Room'] @ ([kitchen]->[connected-to]->['where'])(['where']->[sound]->['indirect-answer'])
  {[kitchen]->[connected-to]->[living-room]}
  {[living-room]->[sound]->[music]}
command> $['where']
  x2 [living-room]
command> $['indirect-answer']
  [music]
```

The top-level unordered collection of ordered atoms is a query graph. Repeating `['where']` gives the two atoms one shared hole, so only mutually consistent rows survive. There is one `where = living-room` binding in the completion; its temporary output-Percept view shows `x2` because each of the two participating source Concepts supplies that same binding. Pangine does not know what `connected-to` or `sound` mean; the composition comes from the structure I asked for. This same distinction lets a one-step transition stay one step while a syllogism-shaped question can request two relationships explicitly.

### Let repeated experience change relevance and choice

Every `~=` command already represents one experience. If the same thing is experienced twice, I can simply say it twice:

```text
command> ['world'] ~= [morning][birds]
  [birds][morning]
command> ['world'] ~= [morning][birds]
  x2([birds][morning])
command> ['world'] ~= [morning][traffic]
  x2([birds][morning])
  [morning][traffic]
command> ['world'] @ [morning]['answer']
  [birds][morning]
  [morning][traffic]
command> $['answer']
  x2 [birds]
  [traffic]
command> ^['answer']
  [birds]
```

In this prototype, each complete `~=` input acts as one experience boundary. The current answer projection therefore gives `birds` a total of two and `traffic` a total of one. I do not need to add `event-1` and `event-2` Concepts unless those event identities are meaningful to the information itself.

For retained experience, the current implementation keeps the selected Percept, direct source Concept, member relevance, matched view, and complete assignment attached to each completion. Repeating one equal Concept adds default relevance to that member. A direct ordinary subject instead uses the complete Concept itself with default relevance and no selected Percept. Those details preserve useful source boundaries, but they are not yet a general account of how all evidence should matter.

The `x2` on the answer is how this prototype exposes that current total. It is useful to the placeholder `^` choice, but it is not a definition of relevance or a general truth score.

### Ask several sources together

Writing several plain Percepts on the left of `@` selects all of them:

```text
command> ['Alice']['Bob'] @ {[cat]->['shared-sound']}
  {[cat]->[meows]}
  {[cat]->[purrs]}
command> $['shared-sound']
  [meows]
  [purrs]
command> ^['shared-sound']
  [meows]
```

`@` applies after the complete expression on its left. Parentheses are optional, so this is the normal form:

```text
['Alice']['Bob'] @ {[cat]->['sound']}
```

`(['Alice']['Bob']) @ ...` means the same thing, but the parentheses are not needed.

The selected Percepts are the sources. Experiencing an equal Concept twice under `Alice` gives that member relevance two. Experiencing it once under both `Alice` and `Bob` also contributes twice to the current projection.

`^` currently selects the greatest positive weight. An exact tie uses the earliest canonical Concept spelling, which is why `meows` wins the tie above. This is a stable fallback, not a claim that `meows` is inherently more likely.

### Reuse one output identity

The same output Percept keeps one identity throughout a question:

```text
command> ['reviewer'] ~= {[review]->[review]}
  {[review]->[review]}
command> ['shipper'] ~= {[prepare]->[ship]}
  {[prepare]->[ship]}
command> ['reviewer']['shipper'] @ {['same']->['same']}
  {[review]->[review]}
command> ^['same']
  [review]
```

`{['same']->['same']}` asks for one Concept that can occupy both positions. `{['left']->['right']}` would allow independent answers.

### Keep several outputs correlated

The returned rows preserve which values occurred together:

```text
command> ['pairs'] ~= [A]->[D]
  {[A]->[D]}
command> ['pairs'] ~= [B]->[C]
  {[A]->[D]}
  {[B]->[C]}
command> ['rows'] = (['pairs'] @ ['left']->['right'])
  {[A]->[D]}
  {[B]->[C]}
command> $['rows']
  {[A]->[D]}
  {[B]->[C]}
```

The separate `$['left']` and `$['right']` views are convenient columns, but they cannot express that `A` belonged with `D` and `B` belonged with `C`. The value returned by `@` and stored in `rows` can. The Rust `complete_question` API also retains the complete assignment and the exact source evidence behind each row.

The same works when each possible row is itself a graph of several relationships. Here I ask for two paths, then use that grounded result directly as the subject of another question:

```text
command> ['memory'] ~= [A]->[r]->[B]; ['memory'] ~= [B]->[s]->[C]; ['memory'] ~= [X]->[r]->[Y]; ['memory'] ~= [Y]->[s]->[Z]; ['memory'] @ (['start']->[r]->['middle'])(['middle']->[s]->['end'])
  {[A]->[r]->[B]}{[B]->[s]->[C]}
  {[X]->[r]->[Y]}{[Y]->[s]->[Z]}
command> (['memory'] @ (['start']->[r]->['middle'])(['middle']->[s]->['end'])) @ (['next-start']->[r]->['next-middle'])(['next-middle']->[s]->['next-end'])
  {[A]->[r]->[B]}{[B]->[s]->[C]}
  {[X]->[r]->[Y]}{[Y]->[s]->[Z]}
command> $['next-end']
  [C]
  [Z]
```

The complete value preserves `A-B-C` and `X-Y-Z`; it does not invent `A-B-Z` or `X-Y-C`. Its canonical single-expression spelling is `({[A]->[r]->[B]}{[B]->[s]->[C]})({[X]->[r]->[Y]}{[Y]->[s]->[Z]})`. Parentheses retain each ordinary graph Concept as one member, and that text parses back into the same Concept. Parentheses around the first `@` make its complete result the subject of the second `@`. The first result's clause evidence and source-Concept relevance remain available in its Rust `CompletionResult`; they are not encoded into the grounded language value passed to the next question. Assigning the value first is still valid when I want a named retained source.

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

Asking both sources retains the two exact answers. The unrelated lint Concept does not match the fixed `full-test` Concept:

```text
command> ['Maintainer']['Legacy-note'] @ {[full-test]->['route']}
  {[full-test]->[cargo]}
  {[full-test]->[cli-runner]}
command> $['route']
  [cargo]
  [cli-runner]
command> ^['route']
  [cargo]
```

Pangine does not infer that the maintainer is more authoritative, that the legacy note is older, or that either route should overwrite the other. `^` only applies the current deterministic tie rule.

The caller can instead choose which source to ask:

```text
command> ['Maintainer'] @ {[full-test]->['maintainer-route']}
  {[full-test]->[cli-runner]}
command> $['maintainer-route']
  [cli-runner]
command> ^['maintainer-route']
  [cli-runner]
```

Pangine answers from the selected Percepts' direct subconcepts. The source names remain opaque, and source selection is explicit in the question.

### What this walkthrough does not settle

This walkthrough covers the current console surface: canonical composition, explicit multiplicity, relevance-bearing Percept subconcepts, structural completion, direct grounded subjects, explicit query graphs, one-Percept and multi-Percept source selection, correlated rows, shared output identity, decision, scripts, and inspection.

It does not establish how a later decision rule should use accumulated evidence. Persistence, distributed execution, numeric or temporal domain grammar, and richer sampling behavior also remain open. The current signed 64-bit `x` storage gives the prototype exact integer coefficients and reductions within its range, but it remains temporary because structural multiplicity, inversion, and answer support may not ultimately be one quantity.

## Why I built Pangine

I originally came up with Pangine by writing down pieces of information in a semantic shape, asking questions about them, and then reasoning backward from what the grammar should imply. Pangine explores whether experience, retained state, and questions can all use that same small grammar.

Concepts have canonical forms and can be composed without giving their names a built-in ontology. A Percept retains complete Concepts as direct relevance-bearing subconcepts. A question can use both those members and the recursive structure inside them without turning experience into a separate kind of Concept.

The larger question is whether this can become an inference system in its own right. Experience now accumulates complete Concept members and their relevance without committing to a finished relevance model. The deterministic `^` rule gives Pangine a repeatable placeholder for choosing among answer candidates while I work toward a better understanding of what information the decision should use.

## Scaling direction

When I first thought about scaling Pangine, the model was closer to map/reduce than a central database. Canonical form gives Concepts a stable identity, but no Concept needs a permanent machine owner. Each partition can retain flat relevance-bearing member edges from Percepts to canonical Concepts.

The current prototype uses the same `ConceptMap` entry representation for those Percept members and for ordinary unordered Concept members. The intended distributed direction routes a canonical Concept to one member rather than broadcasting it to every member, so repeated experience can add relevance where that Concept lives. A question member could return local results for reduction without user-written event IDs or cross-member duplicate tracking. This is a design direction, not an implemented distributed protocol.

An operating server may keep a densely connected in-memory representation or a disposable lookup to make matching fast. That structure is a cache over the flat canonical member edges, not another source of truth. It can be rebuilt, dropped, or scoped to one partition.

Question evaluation currently snapshots either one direct canonical Concept or the selected Percept members and their relevance before writing outputs. It derives recursive match views and ordered windows, completes each requested relation atom, and joins clauses on shared Percepts. Those structures are disposable query work over the supplied Concepts, not another kind of stored experience.

A direct subject remains map/reduce-friendly in the same sense as a Percept member: it is already one canonical Concept, can be routed to the same partitioned matcher, and contributes one ephemeral source record rather than requiring a new storage model.

This first production implementation enumerates clause matches and joins them in memory. It establishes the behavior and gives us a testable correctness path, but it does not yet establish large-scale retrieval performance. Query planning, result streaming, efficient indexing, partitioned execution, and richer answer reduction remain active research areas.

## Current prototype status

The current implementation is written in Rust. It includes:

- Parsing and canonical formatting of the grammar
- Weakly interned, canonical Concept graphs
- Unified Percept state built from ordinary relevance-bearing Concept members
- Direct-Concept, one-Percept, and unparenthesized multi-Percept question subjects
- Lazy exact recursive matching and ordered windows with explicit Percept holes
- Conjunctive query graphs joined through shared Percept bindings
- Canonical nested unordered Concepts, with explicit `*` and `/` member merging
- Complete correlated rows with source Concepts, source relevance, matched views, and explicit unordered remainders in the Rust API
- Grounded `@` results that can be questioned directly, assigned, canonically serialized, parsed, and questioned again without losing row correlation
- A provisional projection of Percept-member relevance onto answer coefficients
- A deterministic placeholder choice rule behind `^`
- `$['*']` global inspection
- An interactive console, current-behavior tests, explicitly ignored research warnings, and a browser-local WebAssembly workbench

The relationships written in a question change which completions the prototype can find, and Percept-member relevance can currently change which candidate `^` returns. The meaning and representation of decision evidence, sampling semantics, scalable retrieval, persistence, distributed execution, and a general surface form for constructing Concepts from rows are all open work.

To run the complete verification suite:

```sh
cargo test --workspace --all-targets --release
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Detailed checks for provisional scoring, decision, and matcher-boundary behavior are compiled but ignored by that run. To inspect the current research expectations explicitly:

```sh
cargo test --test research --release -- --ignored
```

Those warnings preserve useful examples; they are not compatibility promises.

## Language at a glance

| Form | Meaning |
| --- | --- |
| `[]` | Null or no Concept |
| `[name]` | Named Concept |
| `['memory']` | Named Percept reference |
| `(expression)` | Make the complete expression one operand of its surrounding composition |
| `[A][B]` | Unordered composition |
| `([A][B])([C][D])` | Unordered composition whose two members are complete unordered Concepts |
| `[A]*[B]` | Merge the direct unordered members of both operands |
| `[A]/[B]` | Merge with an inverted right operand |
| `![A]` | Inversion |
| `[A]->[B]->[C]` | One flat ordered composition; canonical output includes braces |
| `x2[A]` | Signed 64-bit integer coefficient on the next complete operand; `x-1[A]` formats as `![A]` |
| `['memory'] = expression` | Replace a Percept with zero or one direct subconcept |
| `['memory'] += expression` | Add to the materialized value, then retain one result subconcept |
| `['memory'] -= expression` | Subtract from the materialized value, then retain one result subconcept |
| `['memory'] *= expression` | Merge unordered members, then retain one result subconcept |
| `['memory'] /= expression` | Inverse merge, then retain one result subconcept |
| `['memory'] ~= expression` | Add one complete Concept as a relevance-bearing subconcept |
| `concept @ expression` | Complete a structural question against one grounded Concept, return grounded row(s), and bind output Percepts |
| `['memory'] @ expression` | Complete against the direct subconcepts of one Percept |
| `['Alice']['Bob'] @ expression` | Complete against several selected sources; parentheses are optional |
| `$operand` | Recursively evaluate every Percept in the operand |
| `^['choice']` | Run the current deterministic choice placeholder |
| `$['*']` | Inspect all live ordinary Concepts through a read-only computed view |

Statements may be separated with semicolons. C-style block comments and C++-style line comments are ignored. Canonical output may differ from valid input syntax. For example, `[A]->[B]` formats as `{[A]->[B]}`.

## Contributing

Reproducible bug reports and focused design discussion are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md) for the current contribution policy.

## Licensing

Pangine is source-available under the [PolyForm Noncommercial License 1.0.0](LICENSE.md). Noncommercial use, modification, and distribution are permitted under its terms; commercial use requires separate permission from APU Software, LLC.

This is not an OSI-approved open-source license. See [NOTICE](NOTICE) for ownership and attribution.

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

Everything below is an actual transcript from `pangine-console`. Text after `command>` is what I typed. The indented lines are exactly what the console printed. The first part is one continuous session, so each operation builds on what came before it.

Pangine does not know what names such as `cat`, `maintainer`, or `full-test` mean. They are ordinary names chosen for the examples. Their application meaning comes from the person or program using the language.

### Start with nothing

```text
command> []
  []
```

`[]` is the null Concept. It means there is no Concept here. It is not a named Concept whose name happens to be empty.

The smallest non-null Concept is a name:

```text
command> [cat]
  [cat]
```

Pangine treats `cat` as an opaque name. It does not assume that a cat is an animal or that the word refers to anything in particular.

### Put Concepts together

Writing Concepts next to each other makes a union. Null contributes nothing to it:

```text
command> [cat][]
  [cat]
command> [cat][dog]
  [cat]
  [dog]
```

The console prints the operands of a union on separate lines. Repeating the same operand preserves multiplicity, which canonical output shortens to a strength coefficient:

```text
command> [cat][cat][dog]
  x2 [cat]
  [dog]
```

Parentheses group an expression but do not create a new kind of Concept. Adjacency can preserve a grouped union as one operand, while `*` flattens both sides before merging them:

```text
command> ([A][B])([A][B])
  x2 [A][B]
command> ([A][B])*([A][B])
  x2 [A]
  x2 [B]
```

The first result contains two copies of the complete `[A][B]` group. The second contains two copies of `A` and two copies of `B`.

`/` is the inverse form of the flattening merge. It merges the left side with an inverted right side:

```text
command> ([A][B])/([A][C])
  [B]
  ![C]
```

The positive and inverted copies of `A` cancel. `B` remains positive and `C` remains inverted.

Inversion can also be written directly:

```text
command> ![cat]
  ![cat]
command> [cat]*![cat]
  []
```

Inversion is part of Concept composition. Pangine does not automatically interpret it as natural-language negation or as evidence that a statement is false.

### Attach relevance

Relevance coefficients prefix the next union operand:

```text
command> 50%x2[cat]x3[dog]
  50%x2 [cat]
  x3 [dog]
```

`50%` is the probability component and `x2` is the strength component attached to `cat`. `x3` applies separately to `dog`. These values participate in deterministic composition and selection. The current values are not calibrated probabilities or confidence claims.

Parentheses matter when one coefficient should apply to a complete group:

```text
command> x2([cat][dog])
  x2 [cat][dog]
```

Here the coefficient belongs to the complete `[cat][dog]` Concept rather than to each name independently.

### Describe relationships

`->` makes a directed Correlation:

```text
command> [cat]->[purrs]
  {[cat]->[purrs]}
```

The console adds braces because `{[cat]->[purrs]}` is the canonical form. Canonical output is valid Pangine that parses back into the same Concept.

Correlations associate to the right:

```text
command> [cat]->[purrs]->[soft]
  {[cat]->{[purrs]->[soft]}}
```

That structure relates `cat` to the complete inner relationship from `purrs` to `soft`.

### Preserve where something came from

An Observation combines an observer with the complete Concept it observed or asserted:

```text
command> ?[alice]:{[cat]->[purrs]}
  ?[alice]:{[cat]->[purrs]}
```

Both sides are ordinary recursive Concepts. Pangine does not give `alice` built-in authority or reliability.

The null observer represents global scope:

```text
command> ?[]:{[cat]->[purrs]}
  ?[]:{[cat]->[purrs]}
```

Several complete Observations can form one Observation-state Concept:

```text
command> <?[alice]:[cat], ?[camera]:[purrs]>
  <?[alice]:[cat], ?[camera]:[purrs]>
```

This collection is unordered and idempotent. Repeating an equal complete Observation changes nothing. An unequal Observation remains a separate entry.

### Give a percept mutable state

Square brackets with a quoted name refer to a percept:

```text
command> ['memory']
  ['memory']
command> $['memory']
  []
```

The first command returns the percept reference itself. `$` recursively evaluates percepts, and this percept has no value yet, so evaluation returns null.

Assignment gives it a value:

```text
command> ['memory'] = {[cat]->[purrs]}
  {[cat]->[purrs]}
command> ['memory']
  ['memory']
command> $['memory']
  {[cat]->[purrs]}
```

The reference remains `['memory']`; evaluating it returns the current value.

Percepts support the same ordinary state operations:

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
command> $['state']
  [B]
```

`+=` adds a union operand, `-=` removes it, `*=` performs a flattening merge, and `/=` performs an inverse merge. The grouped union example above shows where ordinary union addition and flattening merge differ.

### Use scripts and comments

Several statements may share one line. The value of the last statement is printed:

```text
command> ['steps'] = [draft]; ['steps'] += [review]; $['steps']
  [draft]
  [review]
```

Line and block comments are ignored:

```text
command> [cat] // the rest of this line is a comment
  [cat]
command> /* comments can also sit inside a line */ [dog]
  [dog]
```

### Turn input into experience

`~=` experiences a Concept rather than merely assigning it:

```text
command> ['observations'] ~= ?[alice]:{[cat]->[purrs]}
  <?[alice]:[cat], ?[alice]:[purrs], ?[alice]:{[cat]->[purrs]}>
```

The complete Correlation is retained, and its recursively exposed `cat` and `purrs` parts are retained with the same observer. If the input did not have an explicit observer, Pangine would use the null observer.

Replaying the exact same Observation produces the exact same state:

```text
command> ['observations'] ~= ?[alice]:{[cat]->[purrs]}
  <?[alice]:[cat], ?[alice]:[purrs], ?[alice]:{[cat]->[purrs]}>
```

There is no duplicate to clean up later. Replay idempotence belongs to the experience operation itself.

### Ask a question

A percept inside a question pattern is an output position. This asks what can occupy the left side of a Correlation whose right side is `purrs`:

```text
command> ['observations'] @ {['animal']->[purrs]}
  {['animal']->[purrs]}
```

The immediate result is the unresolved question shape. Evaluating the output percept shows its candidates:

```text
command> $['animal']
  x2 [cat]
```

The current structural fold found `cat` through two compatible paths, so it has strength `x2`. That number is a deterministic structural coefficient, not a probability.

`^` is the current greedy choice placeholder. It evaluates the percept and returns the entry with the greatest positive Relevance weight:

```text
command> ^['animal']
  [cat]
```

This is similar in shape to greedy or top-1 decoding: choose the candidate with the largest current score. The similarity stops there. Pangine's coefficients are structural values, not model logits or calibrated probabilities, and `^` does not implement temperature, filtering, or random sampling. Ties currently fall back to allocation order, and if nothing has positive weight the complete evaluated value is returned. Those details remain placeholders rather than settled decision semantics.

### Reuse one output identity

Repeating the same output percept in a question requires one shared answer:

```text
command> ['pairs'] ~= ?[unequal]:{[prepare]->[ship]}
  <?[unequal]:[prepare], ?[unequal]:[ship], ?[unequal]:{[prepare]->[ship]}>
command> ['pairs'] ~= ?[equal]:{[review]->[review]}
  <?[equal]:[review], ?[equal]:{[review]->[review]}, ?[unequal]:[prepare], ?[unequal]:[ship], ?[unequal]:{[prepare]->[ship]}>
command> ['pairs'] @ {['same']->['same']}
  {['same']->['same']}
command> $['same']
  x3 [review]
  [prepare]
  [ship]
command> ^['same']
  [review]
```

`review` satisfies both positions in one complete path. `prepare` and `ship` remain as weaker partial matches, but Pangine rejects any complete path that would give the two occurrences different answers.

Using two different percept names, such as `{['left']->['right']}`, allows the two positions to bind independently.

### Inspect the live ordinary Concepts

The global inspection percept is named `['*']`. Evaluating it creates a transient snapshot of all currently live ordinary Concepts. This is the built-in way to inspect the complete ordinary Concept graph retained by the engine. Here I start a fresh console so the result stays small:

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

The snapshot is read-only and computed when requested. "Live" means retained by a Percept value, another Concept, or an owning host handle. A temporary Concept can disappear after its last handle is released. This view is useful for inspection, but it is not persistence or a serialized database.

### A deeper example: choosing a build policy

This final example starts with another fresh console. It uses only the primitives introduced above.

Suppose a maintainer reports that the full test suite should run through `cli-runner`:

```text
command> ['reports'] ~= ?[maintainer]:{[full-test]->[cli-runner]}
  <?[maintainer]:[cli-runner], ?[maintainer]:[full-test], ?[maintainer]:{[full-test]->[cli-runner]}>
```

Receiving the same report again leaves state unchanged:

```text
command> ['reports'] ~= ?[maintainer]:{[full-test]->[cli-runner]}
  <?[maintainer]:[cli-runner], ?[maintainer]:[full-test], ?[maintainer]:{[full-test]->[cli-runner]}>
```

The maintainer also supplies a lint route:

```text
command> ['reports'] ~= ?[maintainer]:{[lint]->[clippy]}
  <?[maintainer]:[cli-runner], ?[maintainer]:[clippy], ?[maintainer]:[full-test], ?[maintainer]:[lint], ?[maintainer]:{[full-test]->[cli-runner]}, ?[maintainer]:{[lint]->[clippy]}>
```

Now I ask which route belongs on the right side of `full-test`:

```text
command> ['reports'] @ {[full-test]->['reported-route']}
  {[full-test]->['reported-route']}
command> $['reported-route']
  x2 [cli-runner]
  [clippy]
command> ^['reported-route']
  [cli-runner]
```

`cli-runner` matches the complete requested relationship. `clippy` survives as a weaker partial structural candidate. The provisional greedy choice selects `cli-runner`, but the coefficients are still structural weights rather than calibrated certainty.

Now an older note reports a different route:

```text
command> ['reports'] ~= ?[legacy-note]:{[full-test]->[cargo]}
  <?[legacy-note]:[cargo], ?[legacy-note]:[full-test], ?[legacy-note]:{[full-test]->[cargo]}, ?[maintainer]:[cli-runner], ?[maintainer]:[clippy], ?[maintainer]:[full-test], ?[maintainer]:[lint], ?[maintainer]:{[full-test]->[cli-runner]}, ?[maintainer]:{[lint]->[clippy]}>
command> ['reports'] @ {[full-test]->['reported-route']}
  {[full-test]->['reported-route']}
command> $['reported-route']
  x2 [cli-runner]
  [clippy]
  x2 [cargo]
```

Pangine retains both complete reports and their observers. It does not infer that `maintainer` is more authoritative than `legacy-note`, that one report is newer, or that `cli-runner` should overwrite `cargo`. Calling `^` here would merely apply the current positive-weight greedy rule and its allocation-order tie break. That would not be a justified conflict-resolution policy.

An application can instead choose one complete policy explicitly. Here it supplies two self-contained policy states:

```text
command> ['policy-v1'] = <?[maintainer]:{[full-test]->[cargo]}, ?[maintainer]:{[lint]->[clippy]}>
  <?[maintainer]:{[full-test]->[cargo]}, ?[maintainer]:{[lint]->[clippy]}>
command> ['policy-v2'] = <?[maintainer]:{[full-test]->[cli-runner]}, ?[maintainer]:{[lint]->[clippy]}>
  <?[maintainer]:{[full-test]->[cli-runner]}, ?[maintainer]:{[lint]->[clippy]}>
command> ['policy-v1'] @ {[full-test]->['route-v1']}
  {[full-test]->['route-v1']}
command> ^['route-v1']
  [cargo]
command> ['policy-v2'] @ {[full-test]->['route-v2']}
  {[full-test]->['route-v2']}
command> ^['route-v2']
  [cli-runner]
```

Pangine answers within whichever complete state the caller selected. The names `policy-v1` and `policy-v2` are opaque. Pangine does not compare their spelling, choose a current version, or define a revision system.

### What this walkthrough does not settle

This walkthrough covers the Pangine surface available in the interactive console apart from the full `help` text. It demonstrates canonical composition, relevance, percept state, experience, questions, shared output identity, selection, scripts, and snapshots.

It does not establish calibrated probabilities, automatic conflict resolution, persistence, authorization, a numeric or temporal domain grammar, an LLM integration, or a production revision API.

## Why I built Pangine

I originally came up with Pangine by writing down pieces of information in a semantic shape, asking questions about them, and then reasoning backward from what the grammar should imply. Pangine explores whether experience, retained state, and questions can all use that same small grammar.

Concepts have canonical forms and can be composed without giving their names a built-in ontology. Experiencing a concept recursively exposes everything it contains, at every level. A question uses literal matches and implied wildcard possibilities to produce weighted answers. The hope is that several partial matches can converge on an answer even when no literal fact was stored.

The larger question is whether this can become an inference system in its own right. Pangine's weighted possibilities could be sampled directly, used to guide an LLM, or combined with other systems. It may end up supplementing current LLM technology, or it may develop into an alternative approach to inference. It is too early to know.

Pangine does not assume that a language model has to perform the inference. If an LLM is involved, it could translate information into and out of Pangine while Pangine retains inspectable state and performs its own reasoning.

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
?[]:{[rain]->[wet_ground]}
```

The second expression reads as "the weather station observed or asserted that rain relates to wet ground." Both the observer and the observation may be any recursive Concept. Pangine does not reserve the observer position for particular kinds of Concept. The third form uses the null observer, written `[]`, for a globally scoped observation. Pangine creates that form when an expression is experienced without an explicit observer. The grammar does not imply observer-specific confidence or independence.

When state contains several distinct Observations, angle brackets identify the collection itself:

```text
<?[weather_station]:[rain], ?[camera]:[wet_ground]>
```

This is one recursive Concept, not an anonymous union that happens to contain Observations. The collection is unordered and idempotent. Repeating an entry does not change it, while ordinary composition still preserves structural multiplicity. Relevance coefficients attach to the next ordinary union operand, so `[A][A][B]` can be written `x2[A][B]`, and `x2[A]x3[B]` weights two operands independently. Parentheses only group: `x2([A][B])` applies one coefficient to the entire grouped Concept. Braces remain the canonical boundary for Correlation.

A named percept holds state. Assignment stores a value, experience accumulates what has been observed, a question binds output percepts, and evaluation materializes their current values.

```text
['memory'] ~= {[cat]->[purrs]}
['memory'] @ {['animal']->[purrs]}
$['animal']
```

The first statement experiences a correlation. The second asks which experienced concepts can occupy the left side of that shape. The third evaluates the resulting ranked candidates.

The same output percept keeps one identity throughout a question pattern. For example, `{['same']->['same']}` asks for one Concept that can occupy both positions, while `{['left']->['right']}` allows two independent answers. When a projection would bind repeated occurrences to different Concepts, Pangine discards that projection. Partial projections can still contribute lower-weight possibilities, so this rule preserves the difference between a candidate that fits the whole shared shape and candidates that fit only one part.

Every concept inside an experience is also experienced. Nothing privileges the root or any other level. In theory, this implies every recursive combination of literal and wildcard structure. In practice, that closure is too large to store, so questions fold the compatible projections together when they are needed. These abstraction levels are somewhat like hidden features in a neural network, except they come from the grammar and remain possible to inspect.

The hard part is relevance. Direct evidence, many independent hints at different levels of abstraction, and a generic wildcard possibility should not all have the same weight. The current projection weights are deterministic, but they are not calibrated probabilities and they are not the final answer.

## Scaling direction

When I first thought about scaling Pangine, the model was closer to map/reduce than GPU acceleration. Canonical form gives concepts a stable identity, but no concept needs a permanent machine owner. Distributed entities could each hold a roughly even subset of relevance. A percept operation could unfold across those entities, perform the local work, and then reduce the partial candidate weights.

Changing how relevance is divided should not change the answer. If an entity goes down, Pangine should keep working with less relevance instead of breaking the model. With enough entities and reasonably balanced relevance, that failure becomes a relatively small change in the available evidence. Rare or important evidence may still need replication.

I have now tested one small part of this idea. I split observations into different groups, folded each group separately, and combined the resulting Concepts. Every grouping and arrival order produced the same canonical result as folding all of the observations together. This happened inside one Pangine engine, so it is not distributed execution yet.

I do not want a later evidence reducer to repair replay duplication. Replay handling now belongs inside `~=` while the Observation still has an observer and a complete recursive payload. Pangine treats every complete canonical Observation as an element of a set. Adding the same Observation again changes nothing, while a different Observation is retained without `~=` inspecting why its payload differs. The same rule applies when an Observation is a recursive part of a larger experience. An unwrapped experience uses the global/null observer and follows the identical rule. Unlike first-write or last-write replacement, set union produces the same state in every arrival order. The later meaning of compatible, inverted, or conflicting observations remains separate from ingestion.

The recursive closure still has to remain implied. Sending every wildcard permutation over the network would replace a storage problem with a messaging problem. Distributed execution is only a design direction today. GPU acceleration could still be useful inside any of the workers.

## Current status

The current implementation is written in Rust. It includes:

- Parsing and canonical formatting of the grammar
- Weakly interned, canonical concept graphs
- Engine-owned percept state and relevance operations
- Recursive, observer-scoped experience sets with idempotent replay
- Lazy recursive wildcard projection, shared repeated-output bindings, and ranked question results
- An interactive console, a progressive Pangine-language walkthrough, and compatibility tests

The relevance model, decision and sampling semantics beyond the provisional greedy `^` choice, persistence, model integration, and language bindings are still open work. Pangine does not currently include an LLM, vector database, llama.cpp integration, or Python package.

To run the complete verification suite:

```sh
cargo test --all-targets --release
```

## Language at a glance

| Form | Meaning |
| --- | --- |
| `[]` | Null or no concept |
| `[name]` | Named concept |
| `['memory']` | Named percept reference |
| `(expression)` | Grouping |
| `[A][B]` | Union |
| `[A]*[B]` | Flattening merge |
| `[A]/[B]` | Merge with an inverted right operand |
| `![A]` | Inversion |
| `[A]->[B]` | Directed correlation |
| `?[observer]:[observation]` | Observation made or asserted by an observer |
| `?[]:[observation]` | Observation in global scope |
| `<?[observer]:[A], ?[]:[B]>` | Multi-entry Observation state |
| `50%x2[A]` | Probability and strength relevance on the next operand |
| `['memory'] = expression` | Assign percept state |
| `['memory'] += expression` | Add one union operand |
| `['memory'] -= expression` | Subtract one complete union operand |
| `['memory'] *= expression` | Flatten and merge into percept state |
| `['memory'] /= expression` | Flatten and inversely merge into percept state |
| `['memory'] ~= expression` | Insert an experience and its recursive observations |
| `['memory'] @ expression` | Bind outputs and return the unresolved question shape |
| `$operand` | Recursively evaluate every percept in its operand |
| `^['choice']` | Apply the provisional positive-weight greedy choice |
| `$['*']` | Inspect all live ordinary Concepts through a read-only computed view |

Statements may be separated with semicolons. C-style block comments and C++-style line comments are ignored. Canonical output may differ from accepted input syntax; for example, `[A]->[B]` formats as `{[A]->[B]}`.

## Contributing

Reproducible bug reports and focused design discussion are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md) for the current contribution policy.

## Licensing

Pangine is source-available under the [PolyForm Noncommercial License 1.0.0](LICENSE.md). Noncommercial use, modification, and distribution are permitted under its terms; commercial use requires separate permission from APU Software, LLC.

This is not an OSI-approved open-source license. See [NOTICE](NOTICE) for ownership and attribution.

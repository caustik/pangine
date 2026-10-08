# Pangine WebAssembly binding

This crate is the thin browser host behind the pangine.com workbench and demos. Pangine still owns parsing, state, canonical formatting, and operations. The binding runs Pangine syntax and console commands, returns answer inspections as JSON, seeds sampling, divides memory among partitions, and exposes a disposable JSON graph view for presentation.

From the `pangine.com` sibling repository, regenerate the browser runtime with:

```sh
npm run runtime:build
```

The visualization retains up to 24 recent command result handles so standalone Concepts can remain visible on its canvas. This is disposable host lifetime rather than persistent semantic knowledge. Because `${*}` deliberately reports ordinary Concepts kept alive by host handles, the workbench history is visible through that inspection operation until the session is reset.

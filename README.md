# Mundis

Mundis is a hyperrealistic civilization simulator focused on watching history
unfold. It simulates an entire planet, including geography, climate, population,
settlement, culture, and the rise and fall of civilizations.

The first version is intentionally text-first. Mundis is less a conventional
game than an observer experience: choose initial parameters, run a deterministic
simulation, and read a detailed chronicle of the world that emerges.

## Direction

- Simulation quality comes before graphics.
- The first interface is a Rust-native CLI chronicle.
- The simulation advances in monthly ticks.
- Worlds are generated procedurally from explicit seeds.
- Geography is represented as a generated region graph for speed, causality,
  and explainability.
- Structured history events are the source of truth.
- Text, JSON, and Markdown are export/render formats derived from events.

## Technology

- Rust workspace for the engine and CLI.
- TOML for user-facing run configs, presets, and authored scenarios.
- SQLite save databases, one file per simulation.
- Binary snapshot blobs for internal simulation state inside save databases.
- Seeded RNG streams for reproducible world generation and history.
- Tauri, Svelte, TypeScript, and PixiJS for the desktop Mundis game app.

## First Milestones

1. Project docs and agent guidance.
2. Rust workspace and deterministic engine skeleton.
3. TOML config and preset loading.
4. SQLite save database with binary snapshots.
5. Procedural region-graph world generation.
6. CLI `run`, `inspect`, and `replay` commands.
7. Structured history events and chronicle rendering.
8. Markdown and JSON history exports.
9. Determinism, invariant, and benchmark tests.
10. History inspection, reconstruction, and graphical world browsing.

## Scenario Authoring

Run configs tune the simulation. Scenario files can also author initial
conditions, then let procedural generation fill anything omitted:

```powershell
cargo run -p mundis -- run --seed 42 --scenario scenario.toml --months 24 --export markdown
```

Scenarios can layer over a base config:

```powershell
cargo run -p mundis -- run --config base.toml --scenario scenario.toml
```

Precedence is `defaults < --config < scenario [simulation] < CLI flags`.

Minimal scenario example:

```toml
[simulation]
months = 24

[simulation.world]
regions = 3

[[regions]]
id = "coast"
name = "Bright Coast"
climate = "temperate"
biome = "grassland"
resources = ["fish", "salt"]
carrying_capacity = 2500
neighbors = ["hills"]

[[regions]]
id = "hills"
name = "Copper Hills"
climate = "arid"
biome = "desert"
resources = ["copper"]
carrying_capacity = 900
neighbors = ["coast"]

[[cultures]]
id = "mariners"
name = "Mariners"
origin_region = "coast"
traits = ["maritime", "mercantile"]

[[settlements]]
id = "harbor"
name = "First Harbor"
region = "coast"
culture = "mariners"
population = 720

[[background_events]]
id = "landing"
summary = "The first ships landed on Bright Coast."
tags = ["origin"]
regions = ["coast"]
settlements = ["harbor"]
cultures = ["mariners"]
```

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

## History Inspection

Saved runs live in a single `.mundis` SQLite database per simulation. Mundis stores sparse
monthly snapshots (month 0, every `history.snapshot_interval_months`, and the final month) and
reconstructs any other month by replaying forward from the nearest snapshot.

```powershell
# Create a saved run
cargo run -p mundis -- run --seed 42 --months 24 --save smoke.mundis --export markdown

# Filter events
cargo run -p mundis -- inspect events --save smoke.mundis --from 1 --to 12 --tag polity --export json

# Reconstruct state at a month (including months without an exact snapshot)
cargo run -p mundis -- inspect state --save smoke.mundis --month 12 --export json

# Entity history (generic subject syntax)
cargo run -p mundis -- inspect entity --save smoke.mundis --subject region:0

# Entity aliases
cargo run -p mundis -- inspect region --save smoke.mundis --id 0 --export json
cargo run -p mundis -- inspect settlement --save smoke.mundis --id 1
cargo run -p mundis -- inspect polity --save smoke.mundis --id 0
cargo run -p mundis -- inspect culture --save smoke.mundis --id 0

# Causal chain around a structured event link (use an event id from the save)
cargo run -p mundis -- inspect chain --save smoke.mundis --event-id 12 --depth 2 --export json

# Export the full chronicle from a save
cargo run -p mundis -- replay smoke.mundis --export markdown
```

Subject filters use `region:0`, `settlement:1`, `polity:2`, `culture:3`, or
`population-group:4`. Event types and severities match the kebab-case names used in JSON exports.
```

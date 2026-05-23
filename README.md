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
- TOML for user-facing run configs and presets.
- SQLite save databases, one file per simulation.
- Binary snapshot blobs for internal simulation state inside save databases.
- Seeded RNG streams for reproducible world generation and history.

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

# Agent Guidance

Mundis is a simulation-first project. Every change should protect the quality,
determinism, and inspectability of the historical simulation.

## Product Principles

- Prioritize simulation depth over graphics.
- Prefer text, logs, reports, and inspectable state before visual polish.
- Keep history non-interactive for now: users configure initial conditions and
  watch the world evolve.
- Make procedural generation the default for names, regions, events, cultures,
  and other world content.
- Treat structured events as canonical history. Renderers may produce plain
  text, JSON, or Markdown, but exports must not become simulation state.

## Technical Direction

- Use Rust for the engine and CLI.
- Use monthly ticks as the default simulation granularity.
- Model the planet as generated regions connected in a graph.
- Use TOML for user-facing configs and presets.
- Use one SQLite save database per simulation.
- Store internal snapshots as binary blobs in the save database.
- Use explicit seeded RNG so the same seed and config reproduce the same run.

## Development Expectations

- Add tests before behavior changes whenever practical.
- Keep engine logic independent from CLI rendering.
- Keep storage independent from simulation rules.
- Prefer small, typed APIs over stringly-typed data flow.
- Verify determinism when touching generation, ticking, events, or storage.
- Avoid adding a graphical viewer until the chronicle experience is compelling.

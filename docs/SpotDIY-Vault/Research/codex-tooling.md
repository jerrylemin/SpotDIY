# Codex tooling research

Date: 2026-08-30

## Primary sources

- [Graphify](https://github.com/Graphify-Labs/graphify)
- [CodeGraph](https://github.com/colbymchenry/codegraph)
- [Ponytail](https://github.com/dietrichgebert/ponytail)
- [Karpathy guidelines](https://github.com/multica-ai/andrej-karpathy-skills)
- [Matt Pocock skills](https://github.com/mattpocock/skills)
- Local Codex skill catalog under `C:\Users\Administrator\.codex\skills` and `C:\Users\Administrator\.agents\skills`.

## Current setup behavior

The local environment has Graphify CLI `0.8.18` on PATH and the project already has the relevant local skills for Ponytail, Karpathy guidelines, code review, TDD, research, implementation, domain modeling, and handoff. The required project bootstrap skill is now present at `.agents/skills/spotdiy-session-bootstrap/SKILL.md`.

CodeGraph is now installed from `@colbymchenry/codegraph@1.6.0` and initialized for the repository. The first index reported 37 files, 196 nodes, 370 edges, a WAL-backed backend, and an up-to-date status. The heavy `.codegraph/` directory remains ignored. Graphify project integration is also installed with the current CLI syntax `graphify install --project --platform codex`; it added the project skill, a small `AGENTS.md` integration, and a project hook. Derived `graphify-out/` data remains ignored.

## Rejected alternatives

- Copying Claude-only skill files into Codex without inspecting metadata was rejected because it can create duplicate or incompatible installations.
- Treating Graphify and CodeGraph as interchangeable was rejected: Graphify relates code, docs, decisions, and vocabulary; CodeGraph answers precise source symbols, calls, and dependencies.
- Committing generated heavy indexes was rejected because they are local derived state and can contain noisy or machine-specific data.

## Risks

- Upstream installer commands and package names can change; re-check official help before installation.
- A global skill update can overwrite user-managed guidance, so project decisions and the local bootstrap skill remain authoritative.
- Graph indexes may become stale after architecture changes and must never be mistaken for source-of-truth code.

## Version assumptions

- Graphify CLI observed: `0.8.18`.
- CodeGraph: `1.6.0`, installed from the official repository package.
- The local skill catalog is the source of truth for what this Codex runtime can route today; upstream version parity is not claimed without a successful installer/version check.

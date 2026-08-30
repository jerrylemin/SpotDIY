# Tauri 2 Typed IPC Research

## Date

2026-08-30

## Conclusion

Tauri 2 does not include a built-in Rust-to-TypeScript command/schema generator. Its core `invoke<T>` API provides a caller-supplied TypeScript generic, but the command name, argument object, and result are not connected to a generated type map and are not runtime-validated.

For a stable Tauri 2 baseline, use explicit Rust DTOs as the wire contract, generate TypeScript declarations from those DTOs with `ts-rs`, and keep a small hand-written command façade for command names and argument envelopes. Add Zod checks only at boundaries where runtime data may be stale, malformed, or externally controlled. This keeps Rust/Serde authoritative while avoiding a second runtime schema for every ordinary command.

`tauri-specta` is the most complete developer-experience option because its Tauri 2 builder can generate command and event bindings and install the matching invoke handler. However, the Tauri 2-compatible line is currently Specta 2 / tauri-specta 2 pre-release material, not a stable dependency baseline as of this research date.

## Primary sources (URLs)

- [Tauri IPC concept](https://v2.tauri.app/concept/inter-process-communication/)
- [Tauri calling Rust commands](https://v2.tauri.app/develop/calling-rust/)
- [Tauri API `invoke` source at the v2.11.5 tag](https://github.com/tauri-apps/tauri/blob/tauri-v2.11.5/packages/api/src/core.ts)
- [Tauri release page](https://v2.tauri.app/release/) and [Tauri v2.11.5 release](https://github.com/tauri-apps/tauri/releases/tag/tauri-v2.11.5)
- [tauri-specta repository README and compatibility table](https://github.com/specta-rs/tauri-specta)
- [tauri-specta current source `Cargo.toml`](https://github.com/specta-rs/tauri-specta/blob/main/Cargo.toml)
- [tauri-specta current `Builder` source](https://github.com/specta-rs/tauri-specta/blob/main/src/builder.rs) and [`commands!` collection source](https://github.com/specta-rs/tauri-specta/blob/main/src/commands.rs)
- [tauri-specta releases](https://github.com/specta-rs/tauri-specta/releases) and [v2.0.0-rc.25 release](https://github.com/specta-rs/tauri-specta/releases/tag/v2.0.0-rc.25)
- [tauri-specta docs.rs page](https://docs.rs/tauri-specta/latest/tauri_specta/) and [v2.0.0-rc.25 package page](https://docs.rs/crate/tauri-specta/2.0.0-rc.25)
- [Specta stable docs.rs page](https://docs.rs/specta/latest/specta/) and [Specta v2.0.0-rc.25 release](https://github.com/specta-rs/specta/releases/tag/v2.0.0-rc.25)
- [ts-rs repository](https://github.com/Aleph-Alpha/ts-rs) and [ts-rs 12.0.1 docs](https://docs.rs/ts-rs/latest/ts_rs/)
- [Typeshare repository](https://github.com/1Password/typeshare) and [Typeshare docs](https://1password.github.io/typeshare/)
- [Zod repository](https://github.com/colinhacks/zod) and [Zod schema source](https://github.com/colinhacks/zod/blob/main/packages/zod/src/v4/classic/schemas.ts)
- [Specta Zod exporter docs](https://docs.rs/specta-zod/latest/specta_zod/)

## Current API behavior

### Tauri 2 core

The public Tauri path is a Rust function marked with `#[tauri::command]`, registered through `tauri::generate_handler!`, and called from the frontend with `invoke` from `@tauri-apps/api/core`. Tauri documents the command boundary as type-aware on the Rust side: command arguments must deserialize with Serde and successful return values must serialize with Serde.

The wire behavior that matters for typed IPC is:

- Arguments are sent as a JSON object by default. The object keys are the Rust parameter names converted to camelCase; `#[tauri::command(rename_all = "snake_case")]` changes that convention.
- A Rust command parameter can be any type implementing `serde::Deserialize`; a successful return value can be any type implementing `serde::Serialize`.
- If a command returns `Result<T, E>`, an `Ok` value resolves the JavaScript promise and an `Err` value rejects it. The error type must also be serializable.
- The stable JavaScript signature is effectively `invoke<T>(cmd: string, args?: InvokeArgs): Promise<T>`. In the v2.11.5 source, `T` is a caller-provided generic and `cmd` remains an unconstrained string. The core API does not generate a command-name union, argument map, result schema, or runtime validator.
- The default JSON path is not the whole IPC surface: Tauri also exposes binary-oriented argument/response paths. Those need a deliberately specified wire contract rather than being treated as ordinary JSON DTOs.

That means this is compile-time guidance only:

```ts
invoke<ExpectedResult>("some_command", { input });
```

The compiler does not verify that `"some_command"` exists, that `{ input }` matches the Rust parameter envelope, or that the returned runtime value really is `ExpectedResult`.

### tauri-specta / Specta compatibility

The official tauri-specta compatibility table is explicit:

| Tauri | Specta | tauri-specta status |
| --- | --- | --- |
| v1 | v1 | Supported by tauri-specta v1 |
| v2 | v1 | Unsupported |
| v1 | v2 | Unsupported |
| v2 | v2 | Supported by tauri-specta v2 |

The v2 `Builder` collects annotated commands/events, exports TypeScript bindings, provides the Tauri invoke handler, and can mount event handling. That is materially more complete than a DTO-only generator: it can keep the generated frontend command surface connected to the Rust command collection. Its builder also exposes choices for error handling, semantic types, Serde “phases,” and bigint conversion, so the generated API is not merely a set of interfaces.

The compatibility caveat is release status. The latest public tauri-specta v2 release observed here is `2.0.0-rc.25`, and GitHub marks it as a pre-release. The current repository `Cargo.toml` is also on a `2.0.0-rc.25` package line while its development dependency set is moving through Specta `2.0.0-rc.26` source. The docs.rs `latest` page still resolves to the older stable `tauri-specta 1.0.2` API, and the rc.25 docs.rs page reports a failed build. Therefore “Tauri 2 + tauri-specta” should not be interpreted as a stable, lockstep package combination yet.

Specta v1’s TypeScript exporter is stable, but its Tauri integration is the combination covered by the unsupported Tauri 2 / Specta 1 row. Specta’s v2 branch is also pre-release. Do not mix `tauri-specta` v1/Specta v1 with Tauri 2, or mix stable Specta v1 with a tauri-specta v2 release candidate.

### Comparison of viable approaches

| Approach | Current status | What is generated/validated | Strengths | Main gap |
| --- | --- | --- | --- | --- |
| Tauri core + `tauri-specta` v2 | Tauri 2-compatible, but pre-release | Command/event bindings, type metadata, and runtime handler integration | Closest to end-to-end typed IPC; least hand-written façade code | RC dependency and upgrade risk; not a stable baseline |
| Explicit Rust DTOs + `ts-rs` | Stable published toolchain; docs show 12.0.1 | TypeScript declarations for Rust types | Rust/Serde remains source of truth; simple and focused | Does not generate command names, argument envelopes, events, or runtime checks |
| Explicit Rust DTOs + Typeshare | Stable published CLI/crate family | TypeScript (and other language) declarations from annotated Rust source | Useful when several non-TypeScript consumers are required | Parser/CLI is not Tauri-aware; still needs command façade and generation discipline |
| Explicit Rust DTOs + handwritten Zod schemas | Zod 4 is stable | Runtime parsing plus inferred frontend types for schemas | Detects malformed/stale values at runtime | Schemas duplicate Rust DTOs unless a separate generator/source-of-truth decision is made |
| Core `invoke<T>` only | Built into Tauri | No generated contract; only caller-side TypeScript annotation | No extra dependency | Does not provide actual cross-side type linkage or runtime validation |

### Recommended stable design: explicit DTOs plus generated TypeScript

Define the IPC payload and result types explicitly in Rust with `Serialize`/`Deserialize`, including the Serde naming and enum representation that define the wire format. Use `ts-rs` to export those DTOs into a designated generated TypeScript directory. `ts-rs` supports `#[derive(TS)]`, `#[ts(export)]`, and explicit export APIs; its documentation also makes clear that Serde compatibility is a supported subset, not a promise that every Serde attribute has an identical TypeScript meaning.

Keep command names and argument envelopes in a small hand-written TypeScript façade. Conceptually:

```ts
export const commands = {
  loadVault: () => invoke<LoadVaultResult>("load_vault"),
  saveEntry: (input: SaveEntryInput) =>
    invoke<SaveEntryResult>("save_entry", { input }),
};
```

The names above are illustrative; the important invariant is that the façade uses the exact registered Rust command name and the exact object key expected by Tauri. This is a narrow manual layer, while the payload/result shapes remain generated. Regeneration should be explicit and checked in CI so stale bindings are caught as a diff.

This design is the best current default when stable dependencies and reviewable contracts matter more than zero handwritten IPC glue. It also leaves an easy migration path to tauri-specta v2 later: the DTOs and wire-shape decisions remain explicit, while the façade can be replaced after the v2 dependency line is stable enough for the project’s release policy.

### Where Zod fits

Zod is a runtime validation library, not a Rust-to-TypeScript IPC generator. Its `parse`/`safeParse` APIs are useful when the frontend must defend against values that may be stale or malformed: persisted vault data after a migration, imported files, user-controlled input, plugin/integration data, or a deliberately versioned IPC boundary.

For ordinary commands whose Rust side already deserializes the request and constructs the response, adding a full duplicate Zod schema for every DTO increases maintenance without creating a new source of truth. A pragmatic split is:

- Rust DTOs and generated TypeScript types for the normal compile-time contract.
- Small Zod schemas for high-risk external or persisted shapes, or for response validation at a boundary where a bad value must fail locally and diagnostically.
- Rust-side validation and authorization remain mandatory; frontend Zod validation is not a security boundary and does not replace Tauri capabilities/permissions.

The Specta Zod exporter is not a stable substitute for this approach: its official docs describe `specta-zod 0.0.3` as active development and not ready for general-purpose use, and the Specta project lists Zod support as partial.

## Rejected alternatives

- **`tauri-specta` v1 with Tauri 2.** Rejected because the project’s official compatibility table marks Tauri 2 + Specta 1 as unsupported.
- **tauri-specta/Specta v2 as the stable baseline.** Rejected for now because the latest public v2 release observed is `2.0.0-rc.25`, explicitly marked pre-release; the current source is already tracking a newer Specta RC development combination. It remains the strongest candidate to re-evaluate after a stable v2 release.
- **Raw Tauri `invoke<T>` without a façade or generator.** Rejected as a typed-IPC strategy because the generic is not connected to the command string or Rust signature. It is still the underlying transport used by the recommended façade.
- **Zod-only as the cross-language contract.** Rejected as the sole strategy because a frontend schema does not generate or constrain the Rust DTO and does not replace Rust deserialization, validation, or authorization. Zod is retained as a selective runtime layer.
- **Specta’s Zod exporter as the stable generator.** Rejected because the official package documentation says it is not ready for general-purpose use.
- **Typeshare as the primary command-level solution.** Not rejected as a DTO generator, but not selected for this IPC surface: it adds a separate annotation/CLI pipeline and, like `ts-rs`, does not know the Tauri command registry, argument envelopes, event mounting, or invoke error behavior. It is a good alternative if multi-language DTO output is a requirement.

## Risks

- **Serde/TypeScript drift.** Generated declarations can only be correct if the generator understands the relevant Serde attributes. Pay special attention to `rename`, `rename_all`, defaults, flattened fields, tagged/untagged enums, optionality, and custom serializers. Treat the actual serialized JSON shape as the contract and add representative fixture or façade tests for non-trivial types.
- **False confidence from erased types.** TypeScript types disappear at runtime. A typed `invoke<T>` call does not inspect the result. Use Zod or another runtime decoder at boundaries where malformed data is a real failure mode.
- **Command and envelope drift.** DTO generation does not catch a typo in a command name or a wrong top-level argument key. Centralize wrappers, use exact command registration names, and test at least the command registration/fixture boundary.
- **Error-shape ambiguity.** A rejected Tauri promise is different from a successfully serialized application-level error envelope. Choose one wire convention per command family and model it explicitly; ensure every Rust error crossing the boundary implements `Serialize`.
- **JSON numeric precision.** Large Rust integers can lose precision when treated as JavaScript `number`. `ts-rs` documents bigint/large-integer configuration, while tauri-specta exposes an explicit bigint-to-number escape hatch. Do not enable a lossy conversion casually; use strings or a deliberate `bigint`/custom representation where required.
- **Bytes and semantic values.** `Uint8Array`, dates, URLs, channels, and binary responses are not interchangeable with ordinary JSON objects. Choose explicit serialized representations and test them. Avoid relying on a generator’s semantic-type convenience until its runtime glue and package versions are pinned and understood.
- **Generated-file lifecycle.** Generated bindings can become stale or trigger unnecessary frontend reloads if written into a watched source path during development. Keep generation deterministic, put output in the project’s intended generated area, and make CI verify that regeneration produces no unexpected diff.
- **Permissions are separate from types.** A perfectly typed wrapper can still call a command that is unavailable under the app’s Tauri capability/permission configuration. Type generation does not provide authorization.
- **Pre-release dependency coupling.** tauri-specta v2 couples Tauri, tauri-specta, Specta, exporters, and macros. RC upgrades can change generated output or builder behavior; use exact lockfile versions if evaluating it experimentally.

## Version assumptions

- Research date is **2026-08-30**. “Stable” means a published, non-pre-release dependency line suitable for a normal project release, not merely a working GitHub branch.
- The official Tauri release page shows Tauri core `2.11.5` and `@tauri-apps/api` `2.11.1` as the current versions observed for this research. The exact application versions are not pinned in this workspace: no project manifest or lockfile was present in the inspected worktree.
- The official stable docs page observed for tauri-specta is `1.0.2`, but that line is for the older Tauri 1/Specta 1 compatibility path. The Tauri 2-compatible tauri-specta line observed is `2.0.0-rc.25`; Specta’s corresponding v2 line is also `2.0.0-rc.25` pre-release material, with the current tauri-specta source moving through Specta `2.0.0-rc.26` development dependencies.
- Specta’s stable docs page observed is `1.0.5`; this does not make it compatible with Tauri 2 through tauri-specta.
- `ts-rs` docs show `12.0.1`; its documentation lists Rust 1.88.0 as the minimum supported Rust version. Pin the actual version in the project lockfile and verify its Serde-compatibility behavior for the DTOs used.
- Typeshare docs show crate `1.0.5`; the official CLI docs show `typeshare-cli 1.13.4`. These are comparison points, not a project recommendation.
- The official Zod repository shows the Zod 4 line, with `v4.4.3` the latest release observed here. Zod is optional runtime validation, not a replacement for Rust DTO generation.
- Version and compatibility claims above are time-sensitive. Re-check the official release pages before adopting tauri-specta v2 or changing generator versions.

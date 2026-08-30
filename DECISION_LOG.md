# SpotDIY decision log

- [ADR-0001](docs/SpotDIY-Vault/ADRs/ADR-0001-tauri-architecture.md): single Tauri 2 application with Rust native core.
- [ADR-0002](docs/SpotDIY-Vault/ADRs/ADR-0002-typed-ipc.md): explicit Rust DTOs plus Zod validation for the bootstrap boundary.
- [ADR-0003](docs/SpotDIY-Vault/ADRs/ADR-0003-unified-source-model.md): unify provider representations by musical work and retain source provenance.
- [ADR-0004](docs/SpotDIY-Vault/ADRs/ADR-0004-local-first-storage.md): local-first SQLite/WAL storage with deterministic portable mode.
- [ADR-0005](docs/SpotDIY-Vault/ADRs/ADR-0005-sqlite-migrations.md): embedded ordered SQLite migrations, WAL safety, and the Plan 02 schema boundary.
- [ADR-0006](docs/SpotDIY-Vault/ADRs/ADR-0006-provider-source-identity.md): typed SpotDIY IDs, provider identity uniqueness, preferred-source integrity, and Spotify metadata-only sources.
- [ADR-0007](docs/SpotDIY-Vault/ADRs/ADR-0007-settings-storage.md): typed ordinary settings in SQLite with a secure-credential boundary and deferred portable startup.
- [ADR-0008](docs/SpotDIY-Vault/ADRs/ADR-0008-local-library-identity-and-reconciliation.md): persistent folder ownership, opaque local identity, watcher recovery, and conservative scan reconciliation.

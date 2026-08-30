# Testing

Frontend tests use Vitest and Testing Library. Rust tests cover migrations,
repositories, domain invariants, local-library path/fingerprint/metadata/artwork
helpers, recursive scanning, transactional reconciliation, identity
preservation, watcher event coalescing/recovery, paging, reveal ownership,
settings, and status IPC. Provider tests use mocks; live integrations are
opt-in. Plan 03 has 53 Rust tests and 18 frontend tests across four files.

The packaged native smoke uses a temporary synthetic folder and proves launch,
add/initial scan, restart persistence, unchanged and forced rescans, watcher
create/rename/delete/restore behavior, identity stability, reveal validation,
folder removal, and preservation of synthetic media. It does not touch real
music folders. The repository has no Playwright browser test project: the
installed CLI reports its version, but `pnpm exec playwright test --list`
returns `unknown command 'test'`. The packaged window was still exercised via
its CDP endpoint; full mocked-IPC screenshot QA remains a follow-up.

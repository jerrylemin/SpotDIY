# Testing

Frontend tests use Vitest and Testing Library. Rust tests cover migrations, repositories, fusion, queue/shuffle algorithms, backup/import rollback, parsers, and media-process output. Provider tests use mocks; live integrations are opt-in. Browser-level Playwright with mocked IPC and a Windows Tauri launch smoke test cover desktop wiring.

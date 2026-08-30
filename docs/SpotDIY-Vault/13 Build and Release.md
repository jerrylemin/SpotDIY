# Build and release

CI targets Windows and runs frozen pnpm install, typecheck, lint, frontend tests/build, Rust fmt, clippy, and tests. Release packaging uses the Tauri NSIS target. Third-party notices must cover Tauri dependencies and media tools. GitHub Releases are not published until explicitly requested.

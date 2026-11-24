# AGENTS.md

Guidance for coding agents working in the `whatshell` repository.

## Project Overview

`whatshell` is a Rust WhatsApp Web CLI for coding agents and terminal workflows. It uses the personal WhatsApp Web multi-device protocol via `whatsapp-rust`, stores a local SQLite message index, and favors predictable JSON/JSONL output for automation.

This is **not** the WhatsApp Business Cloud API. It links like WhatsApp Web and depends on unofficial WhatsApp Web protocol behavior.

## Useful Commands

Run commands from the repository root (`I:\dev\whatshell`).

### Build & Test

| Command | Purpose |
| --- | --- |
| `cargo build` | Build debug binary. |
| `cargo build --release` | Build release binary (also `bun run build`). |
| `cargo test` | Run Rust tests (unit + integration tests in `tests/`). |
| `bun run test` | Run Rust tests and skill-install tests (`cargo test && bun npm/test-install-skills.js`). |
| `bun run test:skills` | Run only the skill-install test. |
| `cargo fmt` | Format Rust code with rustfmt. |
| `cargo clippy --all-targets --all-features` | Run Rust lints; fix warnings before submitting changes. |

### Local CLI / Debug

| Command | Purpose |
| --- | --- |
| `cargo run -- --help` | Show CLI help. |
| `cargo run -- --json --store <path> auth status` | Check auth status against an isolated store. |
| `cargo run -- --json --store <path> doctor` | Run diagnostics without affecting the default store. |
| `cargo run -- --json --store <path> send text --to +15551234567 --message "hello" --dry-run` | Exercise side-effecting command paths safely with `--dry-run`. |

### Packaging / Release

| Command | Purpose |
| --- | --- |
| `bun add -g .` | Install locally with bun; runs `bun npm/install.js` postinstall to build/install the launcher and skills. |
| `bun npm/install.js` | Run the postinstall flow manually. |
| `npm install -g .` | Alternative npm install; the launcher keeps a `node` shebang for npm/CI compatibility. |
| `npm publish --provenance --access public` | Publish package (CI uses OIDC trusted publishing on `v*` tags; see `.github/workflows/release.yml`). |

## Technologies

- **Language:** Rust (Edition 2024), with small bun-run packaging/install scripts (Node-compatible JS).
- **CLI:** `clap` derive-based command parsing.
- **Async runtime:** `tokio`.
- **WhatsApp protocol/runtime:** `whatsapp-rust`, `wacore`, `wacore-binary`, and related SQLite/transport/HTTP client crates.
- **Storage:** SQLite via `rusqlite` (bundled) for session/index data; FTS5 used when available for message search.
- **Serialization/output:** `serde`, `serde_json`; human-readable table output via `comfy-table`.
- **Testing:** `cargo test`, `assert_cmd`, `predicates`, `tempfile`; skill-install tests run with bun.
- **Packaging:** npm-published package (`package.json`, bun 1.4.5 as `packageManager`) with a Node-compatible launcher (`npm/bin/whatshell.js`) and Cargo binary (`src/main.rs`).
- **CI:** GitHub Actions release workflow (`.github/workflows/release.yml`) builds, tests, verifies tag/version, and publishes to npm with provenance.

## Project Layout

- `src/` - Rust CLI implementation.
  - `main.rs` - binary entry point.
  - `cli.rs` - Clap command/argument definitions.
  - `whatshell.rs` - core command handling/application logic.
  - `store.rs` - local store/index/session handling.
  - `config.rs`, `jid.rs`, `lock.rs`, `output.rs`, `extract.rs` - supporting modules.
- `tests/` - integration/CLI tests (`assert_cmd`).
- `npm/` - npm launcher, install scripts, and skill-install tests.
- `skills/whatshell/` - bundled Agent Skill (`SKILL.md`, references, agent metadata).
- `README.md` - user-facing CLI documentation.
- `.github/workflows/release.yml` - tag-driven npm release workflow.

## Best Practices

- **Preserve automation-friendly behavior.** Keep `--json`/JSONL output stable and machine-readable when changing command output. Prefer additive changes over breaking field/shape changes.
- **Treat side effects explicitly.** Commands that send messages, mutate chats/groups/profile/status/blocking, or delete/revoke content are side-effecting. Support `--dry-run` where feasible and keep confirmation/safety expectations clear.
- **Use isolated stores for tests and debugging.** Pass `--store <path>` or `WHATSHELL_STORE` to avoid touching a real personal store. Tests should use temporary directories.
- **Respect the store lock model.** Live WhatsApp commands use a per-store process lock. Do not parallelize live commands against the same store; document lock behavior when relevant.
- **Keep protocol boundaries honest.** Do not expose unreliable WhatsApp Web surfaces as supported. README/SKILL currently call out unsupported/intentional boundaries (newsletters/channels, communities, join-by-invite, full call audio participation).
- **Prefer small, verifiable changes.** This project touches protocol behavior, local persistence, and user-visible CLI output; validate with targeted tests and dry-run/manual-safe commands.

## Guidelines for Changes

- **Documentation:** When changing commands, flags, output, storage paths, or supported surfaces, update `README.md`, `skills/whatshell/SKILL.md`, and relevant files under `skills/whatshell/references/` as needed. Keep examples accurate.
- **CLI changes:** Update `src/cli.rs`, command handling in `src/whatshell.rs`, and tests in `tests/` together. Add or adjust `--dry-run` coverage for new side-effecting commands when possible.
- **Testing:** Run `cargo test` for Rust changes. Run `npm run test:skills` or `npm test` when touching npm packaging/skill installation. For release-related changes, ensure package version/tag expectations in `.github/workflows/release.yml` still hold.
- **Formatting/linting:** Run `cargo fmt` and `cargo clippy --all-targets --all-features` before finalizing Rust changes. Keep code idiomatic and warnings clean.
- **Safety/privacy:** Avoid logging or documenting unnecessary private message content. Prefer summaries/redacted examples in docs, tests, and issue reports. Do not add features that encourage spam, bulk unsolicited messaging, or unsafe automation.
- **Windows development:** Use PowerShell-compatible shell commands for local automation.

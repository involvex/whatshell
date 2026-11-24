# whatshell

`whatshell` is a Rust WhatsApp Web CLI designed for coding agents and terminal workflows. It uses the personal WhatsApp Web multi-device protocol through `whatsapp-rust`, stores a local SQLite message index, and keeps command output predictable for automation.

This is not the WhatsApp Business Cloud API. It links like WhatsApp Web and depends on WhatsApp's unofficial Web protocol behavior.

## Install

Bun is the configured package manager (1.4.5+, including on Termux / Android). Install globally with bun:

```bash
bun add -g whatshell
```

From this directory:

```bash
bun add -g .
```

The `postinstall` script runs with bun, builds the Rust release binary with Cargo, and installs the launcher plus agent skills. npm still works for consumers who prefer it (`npm install -g whatshell`); the launcher keeps a `node` shebang so npm/CI installs remain compatible. You can also use Cargo directly:

```bash
cargo install --path .
```

The bun/npm install also installs the Whatshell Agent Skill globally for coding agents:

- Claude Code: `~/.claude/skills/whatshell`
- Codex and OpenCode agent-compatible discovery: `~/.agents/skills/whatshell`

OpenCode also discovers the Claude-compatible and agent-compatible locations and de-duplicates by skill name, so Whatshell does not install a third OpenCode-native copy by default. Set `WHATSHELL_INSTALL_OPENCODE_SKILL=1` if you explicitly want `~/.config/opencode/skills/whatshell` too. Set `WHATSHELL_INSTALL_CODEX_HOME_SKILL=1` if you explicitly want a `$CODEX_HOME/skills/whatshell` copy. Set `WHATSHELL_SKIP_SKILL_INSTALL=1` to skip skill installation.

## Authenticate

```bash
whatshell auth
```

Scan the QR code in WhatsApp: `Settings > Linked Devices > Link a Device`.

Phone-number pairing is supported when WhatsApp allows it:

```bash
whatshell auth --phone +15551234567
```

QR display adapts to terminal width. On a narrow phone terminal, `--qr auto` (the default) switches to a compact render and falls back to a PNG file when the code cannot fit; `--qr png` always writes an image (default `<store>/pairing-qr.png`, override with `--qr-output <path>`), and `--qr terminal` forces text:

```bash
whatshell auth --qr png --qr-output ./pairing-qr.png
whatshell auth --qr terminal
```

The raw pairing payload is always printed as a fallback, and on Termux a generated PNG is opened with `termux-open` in human TTY sessions.

Status and local logout:

```bash
whatshell auth status --json
whatshell auth logout
```

`auth logout` removes the local session files. If you need to revoke the linked device remotely, remove it from WhatsApp's Linked Devices screen.

## Operational Model

Whatshell keeps a local SQLite index built from WhatsApp Web sync data. Read commands are fastest and most useful after a sync:

```bash
whatshell sync --once --json
whatshell contacts sync --json
```

Use `--json` for automation and preserve returned message IDs, chat JIDs, group JIDs, status IDs, and invite links when you may need to reply, revoke, clean up, or verify an action later.

Live WhatsApp commands use a per-store process lock. Run live commands serially against the same store; if you need an isolated account or test profile, use a separate store:

```bash
whatshell --store ~/.whatshell-work auth
whatshell --store ~/.whatshell-work sync --once --json
```

Use `--read-only` when you want Whatshell to reject side-effecting commands.

## Interactive inbox

Running `whatshell` with no subcommand in a TTY opens a live inbox TUI (chat list, thread, compose). It holds the store lock and keeps a live WhatsApp session until you quit (`q` / Ctrl-C). Agents and scripts should keep using subcommands with `--json` instead.

## Termux / Android

Termux and its forks (for example `com.involvex.termux_app`) are detected automatically from `TERMUX_VERSION` or the standard `$PREFIX/usr` + sibling `$HOME` layout. When detected, Whatshell uses the Termux:API command-line tools if they are installed:

- `termux-wake-lock` / `termux-wake-unlock` keep long-lived sessions (`auth`, `sync --follow`, `listen`, inbox TUI) alive through Android Doze.
- `termux-notification` / `termux-toast` report incoming messages and send results.
- `termux-clipboard-set` copies invite links and downloaded media paths (human TTY sessions only).
- `termux-open` opens downloaded media; pass `--no-open` to keep the file local.

Missing Termux:API tools are silently skipped, and `--json` / `--stream-jsonl` output is never changed. Clipboard, `termux-open`, and notifications only run in human TTY sessions (not with `--json`); agents can also disable everything with `--termux off` or `WHATSHELL_TERMUX=off`, or force it with `--termux on`. Suppress message notifications with `--no-notify` on `listen` or `sync`. `whatshell doctor --json` reports `termux`, `termux_api`, and the detected `termux_commands`.

Recommended setup: bun 1.4.5 or newer (`bun upgrade`), a Rust toolchain and C compiler (`pkg install rust clang binutils`), and the Termux:API app for the integrations above. For media in shared storage, run `termux-setup-storage` and pass an output path under `~/storage/`. Disable Android battery optimisation for Termux when leaving `sync --follow` or `listen` running; the wake lock prevents CPU sleep but cannot stop the system from killing the app.

For pairing on a narrow screen, prefer `whatshell auth --qr png` and view the saved image with `termux-open`; the compact text renderer is used automatically when the terminal is wide enough.

On a narrow phone screen the inbox TUI shows one pane at a time: `Tab` or `Space` switches panes, `i` or `Enter` composes, `?` opens the key help, and `Esc Esc` or `Shift+Q` quits (soft keyboards have no reliable Ctrl key).

## Shell completions

```bash
whatshell completions bash
whatshell completions pwsh
whatshell completions bash --install
whatshell completions pwsh --install
```

`--install` appends (or refreshes) a managed block in `~/.bashrc` or your PowerShell `$PROFILE`. Use `--dry-run` to preview without writing.

## Whatshell Commands

Sync and listen:

```bash
whatshell sync --once --json
whatshell sync --follow --stream-jsonl
whatshell listen --stream-jsonl
```

Send messages:

```bash
whatshell send text --to +15551234567 --message "Build finished" --json
whatshell send file --to 120363000000000000@g.us --file ./report.pdf --caption "Latest report"
whatshell send react --to +15551234567 --id MESSAGE_ID --reaction "+1"
whatshell send poll --to +15551234567 --question "Ship?" --option Yes --option No --json
whatshell send location --to +15551234567 --latitude 12.9716 --longitude 77.5946 --name Bengaluru --json
whatshell send contact --to +15551234567 --name "Ops Phone" --phone +15551234567 --json
```

Query the local index:

```bash
whatshell chats list --json
whatshell messages list --chat +15551234567 --limit 20 --json
whatshell messages search "deployment failed" --limit 10 --json
whatshell messages context --chat +15551234567 --id MESSAGE_ID --before 5 --after 5 --json
whatshell messages reply --chat +15551234567 --id MESSAGE_ID --message "Looking now" --json
whatshell contacts sync --json
whatshell contacts list --json
whatshell contacts search "Devansh" --json
whatshell contacts check +15551234567 --json
whatshell contacts info +15551234567 --json
whatshell groups list --json
whatshell presence typing --chat +15551234567 --json
whatshell chats mark-read --chat +15551234567 --json
whatshell media list --chat +15551234567 --json
whatshell export messages --chat +15551234567 --format jsonl --output ./chat.jsonl
whatshell export analytics --json
```

Message actions:

```bash
whatshell messages edit --chat +15551234567 --id MESSAGE_ID --message "Edited text" --json
whatshell messages revoke --chat +15551234567 --id MESSAGE_ID --json
whatshell messages delete-for-me --chat +15551234567 --id MESSAGE_ID --json
whatshell messages star --chat +15551234567 --id MESSAGE_ID --json
whatshell media download --chat +15551234567 --id MESSAGE_ID --output ./downloads
```

Groups, profile, and status:

```bash
whatshell groups create --subject "Build Room" --participant +15551234567 --dry-run --json
whatshell groups add --jid 120363000000000000@g.us +15551234567 --json
whatshell groups set-subject --jid 120363000000000000@g.us --subject "Build Room 2" --json
whatshell groups set-description --jid 120363000000000000@g.us --description "Release coordination" --json
whatshell groups set-description --jid 120363000000000000@g.us --clear --json
whatshell groups lock --jid 120363000000000000@g.us --json
whatshell groups ephemeral --jid 120363000000000000@g.us --seconds 86400 --json
whatshell profile set-about "Available through whatshell" --json
whatshell blocking is-blocked +15551234567 --json
whatshell status text --message "Working" --to +15551234567 --json
whatshell status revoke --id STATUS_MESSAGE_ID --to +15551234567 --json
```

## Current WhatsApp Web Surface

Implemented now:

- Auth: QR and phone-number pairing, status, local logout.
- Read/index: history sync, live listen, all-message listing, chat filters, FTS search, show, context, JSON/JSONL/CSV export, analytics.
- Reply/send: text, quoted replies, file/media upload, reactions, polls, locations, contact cards.
- Message actions: edit own messages, revoke, delete-for-me, star/unstar.
- Media: list indexed media and download/decrypt stored media messages.
- Contacts: indexed contact/chats list, live number registration check, live contact info lookup.
- Chats: list, archive/unarchive, pin/unpin, mute/unmute, mark read/unread, delete chat.
- Groups: list joined groups, group info/invite info, invite link/reset, leave, create, rename, description, participant add/remove/promote/demote, approval requests, lock/unlock, announcement mode, disappearing messages, member-add mode.
- Presence: online/offline, typing, recording, paused indicators.
- Profile: set push name, set about text, set/remove own profile picture.
- Blocking: list, block, unblock, check blocked state.
- Status/stories: text, image, video, revoke with explicit recipient lists.
- Packaging: Rust binary with npm launcher/install script.

Remaining parity boundaries:

- MCP server mode for direct Claude/Codex tool registration.
- Newsletters/channels and communities are intentionally not exposed until the underlying protocol calls can be made reliable.
- Joining groups by invite is intentionally not exposed until it is proven in a reversible live test.
- Full voice/video call audio participation is not realistically supported by current public WhatsApp Web protocol libraries.
- Link previews and stickers are best-effort through normal media/text sends; WhatsApp may derive previews client-side or server-side depending on account and protocol behavior.

Diagnostics:

```bash
whatshell doctor --json
whatshell doctor --connect
```

## Storage

By default, `whatshell` stores session and index data in the platform data directory. Override it per command or with `WHATSHELL_STORE`:

```bash
whatshell --store ~/.whatshell-work auth
WHATSHELL_STORE=~/.whatshell-work whatshell auth status --json
```

Files:

- `session.db`: WhatsApp Web session managed by `whatsapp-rust`
- `index.db`: local chat/message index with SQLite FTS5 when available
- `LOCK`: process lock for commands that touch the live WhatsApp session
- `media/`: local media downloads

## Research Basis

The command model borrows from the strongest public WhatsApp CLI and agent-oriented projects:

- `vicentereig/whatsapp-cli`: personal WhatsApp Web CLI patterns for Claude/Codex workflows
- `tulir/whatsmeow`: mature Go WhatsApp Web implementation and operational patterns
- `WhiskeySockets/Baileys`: TypeScript WhatsApp Web protocol coverage
- `jlucaso1/whatsapp-rust`: Rust WhatsApp Web client used as the runtime protocol implementation

## Notes

WhatsApp Web automation can break when WhatsApp changes its private protocol. Keep the linked device limited to accounts where this automation is acceptable, and avoid sending spam or high-volume automated traffic.

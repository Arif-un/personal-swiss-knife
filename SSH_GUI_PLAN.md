# Swiss Knife — SSH GUI Implementation Plan

A Termius-style cross-platform SSH client built as a tool inside the existing
Tauri + React 19 "Swiss Knife" app. The existing Pull Requests tool stays; SSH
is added as a sibling sidebar entry.

## Locked spec

| Decision | Choice |
|----------|--------|
| Capabilities (v1) | Connection manager + full interactive embedded terminal + command builder (run *or* copy) |
| SSH engine | `russh` (pure Rust) — no system `ssh` dependency, identical on all OSes |
| Terminal | xterm.js + real remote PTY (vim/htop/colors work) |
| Sites | 2-way sync with `~/.ssh/config` (read + write-back) **and** app's own hosts |
| Auth | SSH keys + ssh-agent only. Key passphrases stored in OS keychain |
| v1 extra features | ProxyJump (jump host) + local `-L` port-forward |
| Deferred | SFTP, `-R`/`-D` forwards, one-off remote command, host folders/groups, key generation |
| OS targets | Linux, macOS, Windows |
| UX | Termius: sidebar host list + tabbed terminals |

### Defaults adopted (not separately confirmed)
- Multiple concurrent sessions in **tabs**.
- **Flat** host list in v1 (groups later).
- **Host-key trust**: on first connect, show fingerprint, prompt to trust, persist
  to `~/.ssh/known_hosts`. russh requires an explicit decision — cannot be skipped safely.
- App-owned hosts persisted as JSON in the Tauri app-data dir; presented in one merged
  list with parsed `~/.ssh/config` hosts (each row tagged by source).
- Keychain access via the Rust `keyring` crate.
- "Build command" = generate the equivalent `ssh -J … -L …` string for copy/paste;
  the actual connection always uses russh.

---

## Architecture

```
React (xterm.js terminal, host list, forms, tab bar)
   │  invoke(cmd)                     listen(event)
   ▼                                      ▲
Tauri command layer  ──►  SessionManager (tauri::State)
   │                          │  per-session tokio task owns the russh channel
   │                          │  mpsc: Write(bytes) | Resize(c,r) | Close
   │                          ▼
   │                       russh client ──► remote host  (PTY channel)
   │                          │  ProxyJump: client-over-channel to bastion
   │                          │  -L forward: local TcpListener → direct-tcpip
   ├─ ssh_config::parse / write_back   (~/.ssh/config)
   ├─ known_hosts::check / add         (~/.ssh/known_hosts)
   └─ keyring                          (key passphrases)
```

### Session lifecycle
1. Frontend: click host → `invoke("ssh_connect", { hostId, cols, rows })`.
2. Rust resolves the merged host config, loads key (agent first, then file +
   keychain passphrase), performs ProxyJump if set, then the russh handshake.
3. Host-key check: russh handler's `check_server_key` compares against
   `known_hosts`. Unknown key → emit `ssh://hostkey` event and **await** a oneshot
   channel; frontend shows fingerprint dialog → `invoke("ssh_trust_hostkey", …)`
   resolves the oneshot.
4. On auth: open channel, `request_pty(term, cols, rows)`, `request_shell`. Return a
   `session_id` (uuid string).
5. A per-session task reads channel bytes → emits `ssh://data` `{ sessionId, bytes }`.
6. xterm.js `onData` → `invoke("ssh_write", …)`; resize (ResizeObserver + FitAddon) →
   `invoke("ssh_resize", …)`.
7. `invoke("ssh_disconnect", …)` or remote EOF → emit `ssh://closed`.

The per-session task **owns** the russh channel; Tauri commands never touch the
channel directly — they push messages onto the session's mpsc. This sidesteps
`Send`/lock contention on the async channel.

### ProxyJump
Connect to the jump host, `channel_open_direct_tcpip(targetHost, targetPort)`, take
`channel.into_stream()` (AsyncRead+AsyncWrite), run a second russh client handshake
over that stream. Chainable for multi-hop later.

### Local port-forward (`-L`)
Per forward: spawn a `TcpListener` on the bind addr/port. Each accepted connection →
`channel_open_direct_tcpip(destHost, destPort)` and bidirectional copy. Tracked in
state, listed/removed in a Forwards panel.

### `~/.ssh/config` write-back safety
Parser records the **line range** of each `Host` block. Writer replaces only the
managed block's lines, regenerating from fields, leaving every other byte untouched
(comments, ordering, unrelated hosts preserved). A timestamped backup
(`~/.ssh/config.bak-<n>`) is written before any modification. New app-created hosts
default to the app JSON store; an explicit "Export to ssh config" appends them under
a `# >>> swiss-knife managed` marker section.

---

## Dependencies to add

**Rust (`src-tauri/Cargo.toml`)**
- `russh` (pin a specific version) — SSH client + `keys` module (key load, agent)
- `tokio` (with `rt-multi-thread`, `net`, `io-util`, `sync`, `macros`)
- `keyring` — OS keychain (Secret Service / Keychain / Credential Manager)
- `dirs` — locate `~/.ssh`, app-data dir
- `uuid` (v4) — session ids
- `thiserror` + `anyhow` — errors
- (already present) `serde`, `serde_json`, `tauri`, `tauri-plugin-opener`

**JS (`package.json`)**
- `@xterm/xterm`, `@xterm/addon-fit`, `@xterm/addon-web-links`
- (already present) tanstack router/query, shadcn ui, lucide

**Tauri capabilities**: add the events used (`ssh://*`) to the capability allowlist;
no extra OS permissions needed beyond default (russh does its own TCP).

---

## Command & event API

**Commands**
```
hosts_list() -> Vec<Host>                 // merged ssh-config + app store
host_get(id) -> Host
host_save(Host) -> id                      // writes to ssh-config block or app store by source
host_delete(id)
ssh_build_command(hostId) -> String        // equivalent ssh CLI string

ssh_connect(hostId, cols, rows) -> sessionId
ssh_trust_hostkey(promptId, trust: bool)
ssh_write(sessionId, data)
ssh_resize(sessionId, cols, rows)
ssh_disconnect(sessionId)

forward_start(sessionId, ForwardSpec) -> forwardId
forward_stop(forwardId)
forwards_list(sessionId) -> Vec<Forward>
```

**Events (Rust → JS)**
```
ssh://data     { sessionId, bytes }
ssh://closed   { sessionId, reason }
ssh://hostkey  { promptId, host, fingerprint, algorithm }
ssh://error    { sessionId?, message }
```

**Data model**
```
Host {
  id, source: "ssh-config" | "app",
  alias, hostname, user, port,
  identityFile?: string, useAgent: bool,
  proxyJump?: string,                 // alias or user@host:port
  forwards: ForwardSpec[],
}
ForwardSpec { type: "L", bindAddr, bindPort, destHost, destPort }
```

---

## Frontend structure

- Route `/ssh` added to `src/routes/`, plus a sidebar nav entry (`TerminalIcon`) in
  `AppSidebar.tsx` alongside Home and Pull Requests.
- Components (`src/components/ssh/`):
  - `HostList` — Termius-style sidebar list, connect / edit / delete.
  - `HostForm` — alias, hostname, user, port, identity file, agent toggle,
    ProxyJump, forwards editor.
  - `TabBar` + `TerminalTab` — one xterm instance per session; wires `ssh://data` →
    `term.write`, `term.onData` → `ssh_write`, ResizeObserver + FitAddon → `ssh_resize`.
  - `HostKeyPrompt` — fingerprint trust dialog.
  - `ForwardsPanel` — add/list/stop `-L` tunnels.
  - `CommandPreview` — shows `ssh_build_command` output with a copy button.
- Session state: a small React context (`sessions`, `activeTab`) — xterm instances
  held in refs, not React state.

---

## Build phases

| Phase | Deliverable | Risk |
|-------|-------------|------|
| 0 | Deps + scaffolding: crates, xterm, `/ssh` route, sidebar entry | low |
| 1 | **Spike**: connect one host (agent/key), host-key trust prompt, PTY shell, xterm streaming both ways, resize, disconnect | **high — validates the core** |
| 2 | `~/.ssh/config` read → merged host list → connect from list; multi-session tabs | med |
| 3 | Host add/edit form, ssh-config write-back (block-range + backup), app JSON store, keychain passphrases | med (write-back fiddly) |
| 4 | ProxyJump (client-over-channel) | med |
| 5 | Local `-L` forward + Forwards panel | med |
| 6 | Command builder (build + copy) + Termius polish (theme, layout) | low |
| 7 | Cross-platform pass: Windows ssh-agent named pipe, macOS build, smoke tests | med |

Phase 1 first because the PTY byte-streaming + resize plumbing is the only piece
with real unknowns; everything else is conventional CRUD/UI.

---

## Verification
- Rust unit tests: `ssh_config` parse → edit → write round-trip preserves unrelated
  content; `known_hosts` match/add.
- Manual: connect to a local `sshd` (or container) and a real host; run `vim`/`htop`
  to confirm interactive PTY; resize window; open 2 tabs; add a `-L` forward and hit it.
- Per-OS smoke test before declaring cross-platform done.

## Known risks & mitigations
- **Async host-key callback** ↔ user decision: oneshot channel awaited inside
  `check_server_key`, resolved by `ssh_trust_hostkey`.
- **ssh-config clobber**: block-range replacement + pre-write backup; never reserialize
  the whole file.
- **Windows ssh-agent**: OpenSSH agent named pipe vs Pageant — russh supports the
  named pipe; test early, fall back to key file + keychain passphrase.
- **russh API churn**: pin the version; isolate russh calls behind a thin internal
  module so an upgrade touches one file.
- Remote PTY means **no local ConPTY needed** — the biggest Windows terminal headache
  is avoided entirely.

---

## Prerequisite (still pending)
`pnpm tauri dev` currently fails — Linux build libs not yet installed. Before Phase 0
build/run works, the system deps must land (run once, needs your sudo):
```bash
sudo apt-get update && sudo apt-get install -y \
  libwebkit2gtk-4.1-dev build-essential curl wget file \
  libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
```
Rust toolchain (1.96) and JS deps are already installed.

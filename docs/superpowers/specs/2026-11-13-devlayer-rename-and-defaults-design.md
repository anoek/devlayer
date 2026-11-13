# Rename to devlayer and new defaults: design

## Intent

The tool is used mainly to run AI coding agents (Claude Code, Codex, pi, ...)
against a project. Today that takes a long config file and several flags:
`--new`, `--bind=.`, `--net=host`, `--audio`, plus a list of agent config
directories. The project is renamed to **devlayer**, and those choices become
the defaults. A bare `devlayer` in a project directory starts a fresh layer.
In that layer the project directory is live on the host, the network and
audio work, and the agent's own config/state is shared with the host. Changes
outside the project directory stay in the layer for review
(`status`/`diff`/`accept`/`reject`).

## Goals

1. Clean rename from `sandbox` to `devlayer` everywhere.
2. New defaults: a new layer per run, the current directory bound, host
   network, audio on, and well-known LLM tool paths bound.
3. `--join <name>` replaces `--name`.
4. `--last` becomes directory-aware, and the actions that operate on one
   layer default to it.

## Non-goals

- No fallback, compatibility or migration for `sandbox` names, paths, env
  vars, config files or existing `~/.sandboxes` data. It is a clean
  switchover.
- No automatic cleanup of the growing number of `ephemeral_*` layers. That
  comes in a later iteration, and `devlayer clean` stays as it is.
- No GitHub repository rename or release work.

## Anti-goals

- Never run with `/` bound live by the default directory bind.
- Re-entering an existing layer from a subdirectory must not fail with
  "bind mounts changed" just because the current directory differs.

## Part 1: Rename (clean switchover)

| Old | New |
|---|---|
| binary / crate / lib name `sandbox` | `devlayer` |
| `~/.sandboxes/`, fallback `/tmp/sandboxes-<uid>` | `~/.devlayer/`, fallback `/tmp/devlayer-<uid>` |
| `SANDBOX_*` env vars | `DEVLAYER_*` |
| `.sandbox.conf` / `.sandbox.toml` (cwd and parents) | `.devlayer.conf` / `.devlayer.toml` |
| `~/.config/sandbox/config.(conf\|toml)` | `~/.config/devlayer/config.(conf\|toml)` |
| `/etc/sandbox.(conf\|toml)` | `/etc/devlayer.(conf\|toml)` |
| test layer prefixes `sandbox-coverage-*`, `sandbox-bind-empty-*` | `devlayer-coverage-*`, `devlayer-bind-empty-*` |
| README, CONTRIBUTING, CLAUDE.md, RELEASE_CHECKLIST, Makefile, packaging, vm-testing, cspell, completions, user-facing messages | updated to "devlayer" / "layer" |

User-visible text (help, errors, output, docs) must say devlayer.

Internal Rust identifiers and modules (`Sandbox`, `SandboxSettings`,
`src/sandbox/`, `SandboxLogger`, `sandbox_*` functions and variables, test
fixture names such as `fixture_sandbox.rs`) are **also renamed**
(Sandbox → Layer / devlayer). This is done as a **separate pass**, in its
own commit(s), apart from the user-facing rename.

Both rename passes land with no behavior change, before Part 2.

## Part 2: New behavior

### Choosing which layer to use

The flags `--new`, `--last` and `--join <name>` are mutually exclusive (as
today).

- **`--join <name>`** works exactly like today's `--name`: it uses that
  layer, and creates it if it does not exist. `--name` is removed.
  - *(my call)* The config-file key `name` becomes `join`, and the env var
    becomes `DEVLAYER_JOIN`. Either one acts like `--join` and overrides the
    defaults below. Combining it with `--new`/`--last` is an error, as
    today.
- **`--new`** creates a layer with an auto-generated `ephemeral_<timestamp>`
  name (naming unchanged, so `clean` keeps working).
- **`--last`** is directory-aware:
  - Each layer records the canonical directory it was created in, in its
    `settings.json` (new field, e.g. `created_in`). The value is written when
    the layer is created and kept unchanged when the layer is restarted.
  - Resolution starts at the canonical cwd and walks up through its
    ancestors to `/`. At the first directory that is the `created_in` of one
    or more layers, it picks the newest of those (by creation time). Note:
    `/` counts too, if a layer was created there with `--no-bind-cwd`.
  - If no layer matches, it fails with an error that names the directory and
    suggests `--join <name>` or `devlayer list`.

**Defaults when no selection flag, `join` config or env var is given:**

| Invocation | Default |
|---|---|
| no action (run a command or a shell) | `--new` |
| `status`, `diff`, `accept`, `reject` | `--last` |
| `stop` / `delete` with no patterns and no `--all` | `--last` |
| `list`, `sync`, `clean`, `config` | need no target. They must not fail or create anything when no layer exists for the cwd. |

An explicit flag always wins, for example `devlayer --join foo status` or
`devlayer --last bash`.

### Default scope of `status` / `diff` / `accept` / `reject`

Today these actions, with no patterns, only consider changes under the cwd
(`ChangeEntries::matching` in `src/sandbox/changes/change_entries.rs`). The
cwd is now bound live, so its changes never reach the layer, and that default
would always be empty.

- **`status` and `diff` with no patterns cover the whole layer** (the same as
  `status /` today). Patterns work as they do today.
- **`accept` and `reject` require at least one pattern.** With no patterns
  they fail with an error that suggests `devlayer accept /` (or `reject /`)
  for everything. This protects against an accidental system-wide accept.
  Pattern behavior is unchanged.

Implementation note: today the name is resolved inside `resolve_config`
before `main` dispatches the action. The default now depends on the action,
so resolution has to know the action (or be deferred until it is known).

### Directory bind (`--bind=.` by default)

- By default the canonical current directory is bind-mounted read-write at
  the same path.
- New flag **`--no-bind-cwd`**, with config key `no_bind_cwd` and env var
  `DEVLAYER_NO_BIND_CWD`, turns it off. It is independent of
  `--no-default-binds`.
- **Refuse to start** a new layer when the directory bind is on and the
  canonical cwd is `/`. The error says to use `--no-bind-cwd`. `$HOME` is
  allowed.
- **Re-entering an existing layer** (`--join` / `--last`, running or
  stopped) *(my call)*: the layer's directory bind is fixed when the layer is
  created. `settings.json` records both `created_in` and whether the
  directory bind was on (e.g. `bind_cwd: bool`). On re-entry and on restart,
  both values come from `settings.json`, and the current cwd and the
  `--no-bind-cwd` flag are ignored for this bind. Results:
  - Entering from a subdirectory or another directory does not change the
    layer's bind set, so it does not trip the "bind mounts changed" check in
    `SandboxSettings::validate_against_config`.
  - A layer created at `/` with `--no-bind-cwd` never binds `/` later.
  - Restarting a stopped layer (which rewrites `settings.json` in
    `mount_overlays.rs`) must keep `created_in` and `bind_cwd`.

  Other binds (system, audio, LLM, config/env/CLI) are recomputed and
  validated as they are today.
- *(my call, known limitation)* Config files are still found by walking up
  from the current cwd, not from `created_in`. So `devlayer --join proj`
  run from a different project can load a different `.devlayer.toml` and
  fail validation. This is today's behavior with `--name`. Document it in
  the README rather than restructure config loading.
- Precedence: the directory bind is a layer above the default and LLM binds
  and below config/env/CLI binds. Explicit binds or masks on the same target
  therefore override it.

### Network

- The default for `net` becomes `host`. `--net=none` (or `net="none"` in
  config) disables networking.
- Consequence (intended, noted for the docs): the D-Bus/systemd default
  binds that today depend on host networking are now present by default.

### Audio

- Audio binds (`/dev/snd`, `$XDG_RUNTIME_DIR/pipewire-0`,
  `$XDG_RUNTIME_DIR/pulse`) become part of the default binds, so
  `--no-default-binds` disables them.
- *(my call)* The `--audio` flag, the `audio` config key and
  `DEVLAYER_AUDIO` are removed.

### Default LLM binds

- A new bind layer of well-known AI-tool paths. It is on by default and
  disabled with **`--no-default-llm-binds`** (config key
  `no_default_llm_binds`, env var `DEVLAYER_NO_DEFAULT_LLM_BINDS`). It is
  independent of `--no-default-binds`.
- Each path is bound read-write at the same path, and **only if it exists**
  on the host:
  - `~/.claude`, `~/.claude.json`
  - `~/.codex`
  - `~/.pi`
  - `~/.gemini`
  - `~/.qwen`
  - `~/.aider` *(moved here from the system defaults)*
  - `~/.cursor` *(moved here from the system defaults)*
  - `~/.config/opencode`, `~/.local/share/opencode`
  - `~/.config/goose`
  - `~/.continue`
  - `~/.codeium`
  - `~/.config/github-copilot`
  - `~/.factory`
  - `~/.config/crush`, `~/.local/share/crush`
- Personal binds (ssh keys, gh tokens, docker.sock, herdr) are **not**
  defaults. They stay in the user's config.

### Bind layer order (lowest to highest precedence)

1. Default system binds (`/dev/fuse`, D-Bus/systemd when net=host) plus
   audio. Skipped with `--no-default-binds`.
2. Default LLM binds. Skipped with `--no-default-llm-binds`.
3. Directory bind. Skipped with `--no-bind-cwd`.
4. Config file layers (as today).
5. Environment layer.
6. CLI layer.

## Approaches considered

1. **Rename first, then behavior changes, on this branch (chosen).** Each
   diff is easy to review. The 116-file rename does not hide the logic
   changes.
2. One combined change: less churn in test files, but much harder to
   review and to bisect.
3. Keep the `sandbox` name with a `devlayer` alias, or migrate old data:
   ruled out, because the user wants a clean switchover.

For `created_in`: store it in the existing per-layer `settings.json`
(chosen), instead of a separate index file in the storage dir. An index
could get out of sync with layers that are deleted by hand, and
`settings.json` already exists per layer.

## Testing

- Follow CLAUDE.md: new tests go in `tests/ai_test_*.rs`. Explicit layer
  names use the `devlayer-coverage` prefix. Keep line coverage at the
  project's required level (`make coverage-check`).
- Existing tests that rely on the old defaults (no network, default name
  `sandbox`, `--name`, `--audio`, `.claude`/`.aider`/`.cursor` in the system
  defaults, cwd-scoped `status`/`diff`/`accept`/`reject` with no patterns)
  must be updated to the new behavior. Do not delete them.
- New coverage is needed for: directory-aware `--last` (exact dir, ancestor
  match, nearest ancestor wins, newest wins within a dir, no match →
  error); defaults per action; `--no-bind-cwd`; refusing `/`; re-entering
  from a subdirectory keeps the original directory bind; LLM binds only
  when the path exists, and disabled by the flag; audio disabled by
  `--no-default-binds`; `--join` create-or-use; `join` config/env;
  whole-layer `status`/`diff` with no patterns; `accept`/`reject` with no
  patterns → error; re-entering or restarting keeps `created_in`/`bind_cwd`.

## Left to the builder

- Exact wording of help text and errors.
- How to restructure name resolution so it knows the action.

## Decided after review

- Internal identifiers and modules are renamed, in a separate pass (see
  Part 1).
- Test helpers (the fixture / layer manager in `tests/fixtures/`) are
  updated for `--join` and the new defaults.

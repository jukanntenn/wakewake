# ZCode Hooks

Project-local hook scripts for the ZCode agent. They are **thin adapters** that delegate all formatting/linting to prek — the single source of truth (the `format` and `lint` groups in the workspace prek configs). No formatter mapping lives in these scripts, so they cannot drift from prek/CI.

- `hooks/post_tool_use.py` — PostToolUse (`Edit|Write`): runs `prek run --group format --files <edited>`. prek routes the file to the right project formatter (cargo fmt / prettier --write / ruff / builtin fixers) at the correct cwd. Never blocks; real errors are surfaced to stderr.
- `hooks/stop.py` — Stop: runs `prek run --group lint --all-files` (cargo fmt --check + clippy + prettier --check + eslint + tsc + actionlint + doc-check); on non-zero exit it prints `{"decision":"block","reason":"..."}` once per turn (guarded by the `stopHookActive` flag).

## Why there is no `.zcode/config.json` here

The ZCode client UI and official docs support workspace-scope hooks in `.zcode/config.json`, but the agent runtime on this machine (v2.1.0, WSL server) **strips them unconditionally** — a "security policy" warning (`config_project_hooks.ignored`) is logged and no hook runs. Only hooks in the **user-level** `~/.zcode/cli/config.json` are executed (verified in the markpost project's source and log evidence).

The user-level config therefore points at these scripts via `${ZCODE_PROJECT_DIR}` and guards on file existence, so other workspaces without this directory are unaffected. Changing the runtime behavior would require a ZCode update that honors workspace hooks. Registration (one-time, per machine):

```json
{
  "hooks": {
    "PostToolUse": [
      {
        "matcher": "Edit|Write",
        "hooks": [
          {
            "type": "command",
            "command": "if [ -f \"${ZCODE_PROJECT_DIR}/.zcode/hooks/post_tool_use.py\" ]; then python3 \"${ZCODE_PROJECT_DIR}/.zcode/hooks/post_tool_use.py\"; fi"
          }
        ]
      }
    ],
    "Stop": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "if [ -f \"${ZCODE_PROJECT_DIR}/.zcode/hooks/stop.py\" ]; then python3 \"${ZCODE_PROJECT_DIR}/.zcode/hooks/stop.py\"; fi"
          }
        ]
      }
    ]
  }
}
```

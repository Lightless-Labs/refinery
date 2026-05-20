---
title: "CLI provider subprocess isolation and configuration"
category: integration-issues
tags: [codex, gemini, claude, subprocess, setsid, stdin, process-group, cli-flags]
module: tundish_providers
symptom: "Child CLI processes fail, self-cancel, or corrupt parent terminal state"
root_cause: "Various: missing CLI flags, deprecated flags, terminal inheritance"
date: 2026-03-14
---

# CLI provider subprocess isolation and configuration

## Provider-specific CLI flags

### Codex CLI
- **`--skip-git-repo-check`**: Required. Without it, codex refuses to run outside a trusted git directory: "Not inside a trusted directory and --skip-git-repo-check was not specified."
- **`--sandbox read-only`**: Filesystem restriction
- **`--json`**: JSONL output mode
- **`exec`**: Non-interactive mode (important: this is a subcommand, not a flag)

### Gemini CLI
- **`--allowed-tools` is deprecated**: Causes `FatalCancellationError` with exit code 130. Remove entirely. `--sandbox` + `--approval-mode plan` is sufficient for tool restriction.
- **`--sandbox`**: Enables sandboxing
- **`--approval-mode plan`**: Non-interactive approval
- **`--output-format json`**: JSON output
- **System prompt**: Via `GEMINI_SYSTEM_MD` env var pointing to a temp file (not inline)

### Claude CLI
- **`--tools ""`**: Disables ALL tools (do NOT use `--disallowedTools` blocklist — fragile)
- **`-p`**: Print mode (non-interactive)
- **`--verbose`**: Required for `stream-json` output in print mode
- **`--output-format stream-json`**: JSONL streaming output
- **`--json-schema`**: Inline JSON schema for structured output

## Subprocess isolation (critical for terminal safety)

All child processes MUST be isolated from the parent's terminal:

```rust
cmd.stdin(std::process::Stdio::null());   // Don't inherit TTY
cmd.stdout(std::process::Stdio::piped()); // Capture output
cmd.stderr(std::process::Stdio::piped()); // Capture errors

#[cfg(unix)]
{
    #[allow(unsafe_code)]
    unsafe {
        cmd.pre_exec(|| {
            if libc::setsid() == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

cmd.kill_on_drop(true);
```

### Why each piece matters

| Isolation | Purpose | Without it |
|---|---|---|
| `stdin(Stdio::null())` | Don't give child the TTY on stdin | Child sees `isatty()==true`, enters interactive mode |
| `setsid()` | New session, no controlling terminal | Child opens `/dev/tty`, disables ISIG, breaks Ctrl+C for parent |
| `kill_on_drop(true)` | Kill child when handle dropped | Orphaned child processes survive parent exit |
| `env_clear()` | Clean environment | Child inherits sensitive env vars |

### Why `process_group(0)` is insufficient

`process_group(0)` puts the child in a background process group. Background processes that call `tcsetattr()` should receive `SIGTTOU`. But interactive CLIs **ignore SIGTTOU** (`signal(SIGTTOU, SIG_IGN)`) as a standard practice, so `tcsetattr()` succeeds from the background group anyway.

`setsid()` is the nuclear option: the child has no controlling terminal at all. `open("/dev/tty")` returns `ENXIO`.

## Environment variables

### Security: always `env_clear()` first

```rust
cmd.env_clear();
cmd.env("PATH", sanitized_path());  // Minimal sanitized PATH
cmd.env("HOME", home);              // For credential files
cmd.env("TMPDIR", tmpdir);          // Required on macOS
```

### Provider-specific env vars

- **Claude**: `ANTHROPIC_API_KEY` or `CLAUDE_CODE_OAUTH_TOKEN` (both optional — falls back to `~/.claude.json`)
- **Codex**: `OPENAI_API_KEY` or `CODEX_API_KEY` (optional — falls back to stored credentials)
- **Gemini**: `GEMINI_API_KEY` or `GOOGLE_API_KEY` (optional — falls back to gcloud credentials)

All credentials are optional: if no env var is set, each CLI uses its own stored authentication.

## Cross-references

- `docs/solutions/runtime-errors/ctrl-c-sigint-terminal-isolation.md` — full SIGINT debugging saga
- `docs/solutions/integration-issues/cli-provider-flags-and-sandboxing.md` — original provider flag documentation

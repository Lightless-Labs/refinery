---
title: "Ctrl+C / SIGINT not terminating Rust CLI that spawns interactive child processes"
category: runtime-errors
tags: [sigint, ctrl-c, terminal, isig, setsid, process-group, tokio, tty, tcsetattr, signal-handling]
module: tundish_providers
symptom: "Pressing Ctrl+C echoes ^C but does not terminate the CLI. kill -INT from another terminal works."
root_cause: "Child CLI processes open /dev/tty, call tcsetattr() to disable ISIG on the terminal. With ISIG disabled, Ctrl+C generates no signal — just echoes ^C as data."
date: 2026-03-14
---

# Ctrl+C / SIGINT not terminating Rust CLI that spawns interactive child processes

## Context

ConVerge Refinery is a Rust CLI using tokio (multi-threaded runtime) that spawns interactive AI CLI tools (claude, codex, gemini) as child processes via `tokio::process::Command`. When the user presses Ctrl+C, the process does not stop — `^C` is echoed but no signal is generated.

## Symptom

- Pressing Ctrl+C in the terminal echoes `^C` but the process keeps running
- `kill -INT <pid>` from another terminal DOES terminate the process
- This asymmetry proves the signal disposition is correct — the terminal driver is the problem

## Root Cause

Interactive CLI tools (claude-code, codex, gemini-cli) are designed for interactive terminal use. When spawned as children:

1. They detect a controlling terminal via `isatty()` or by opening `/dev/tty` directly
2. They call `tcsetattr()` to set the terminal into raw mode, which **disables the `ISIG` flag**
3. With `ISIG` disabled, the terminal driver no longer translates Ctrl+C (0x03) into SIGINT
4. The terminal just echoes `^C` as a printable character
5. This state **persists across commands** — subsequent runs inherit the corrupted terminal

This is why `kill -INT` works: it bypasses the terminal driver and sends SIGINT directly to the process.

## What Didn't Work (7 failed attempts)

### 1. `tokio::signal::ctrl_c()` in `tokio::select!`
**Why it failed:** The signal was never generated in the first place (ISIG disabled). Even if it were, the engine kept making progress between await points.

### 2. `tokio::spawn` with `ctrl_c()` + `std::process::exit(130)`
**Why it failed:** Same — no signal to catch. Also, the tokio task might not get polled if the runtime is busy.

### 3. `ctrlc` crate + `std::process::exit(130)`
**Why it failed:** `exit()` runs atexit handlers which try to flush stderr. The spinner thread holds the stderr lock. Deadlock — `exit()` never returns.

### 4. `ctrlc` crate + `libc::_exit(130)`
**Why it failed:** The `ctrlc` crate's pipe+reader thread mechanism didn't fire reliably. Also, no signal to catch.

### 5. Raw `libc::signal(SIGINT, handler)` with `_exit(130)`
**Why it failed:** No signal was ever delivered to handle.

### 6. `sigaction(SIGINT)` with `SA_RESTART`, installed before tokio runtime
**Why it failed:** Still no signal — ISIG is disabled on the terminal.

### 7. `pthread_sigmask(SIG_BLOCK)` + `sigwait()` on dedicated thread
**Why it failed:** `sigwait` blocks forever because the signal is never generated.

### 8. `cmd.stdin(Stdio::null())`
**Why it was insufficient:** Children bypass stdin — they open `/dev/tty` directly.

### 9. `cmd.process_group(0)`
**Why it was insufficient:** Children in background process groups that call `tcsetattr()` get `SIGTTOU`, but interactive CLIs **ignore SIGTTOU** (`signal(SIGTTOU, SIG_IGN)`), so `tcsetattr()` succeeds anyway.

## Working Solution (3 layers)

### Layer 1: `stdin(Stdio::null())`
Prevents children from inheriting the TTY on stdin.

### Layer 2: `setsid()` via `pre_exec`
Creates a new session for each child. The child has **no controlling terminal** — `open("/dev/tty")` returns `ENXIO`. The child physically cannot modify terminal attributes.

```rust
// crates/tundish_providers/src/process.rs
cmd.stdin(std::process::Stdio::null());
cmd.stdout(std::process::Stdio::piped());
cmd.stderr(std::process::Stdio::piped());

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

### Layer 3: Restore ISIG on startup
Previous runs (before the fix) may have corrupted the terminal state. This persists across commands. On startup, detect and repair.

```rust
// crates/refinery_cli/src/main.rs — first thing in main()
#[cfg(unix)]
{
    use std::os::fd::AsRawFd;
    let stderr_fd = std::io::stderr().as_raw_fd();
    #[allow(unsafe_code)]
    unsafe {
        let mut termios: libc::termios = std::mem::zeroed();
        if libc::tcgetattr(stderr_fd, &raw mut termios) == 0
            && termios.c_lflag & libc::ISIG == 0
        {
            termios.c_lflag |= libc::ISIG;
            libc::tcsetattr(stderr_fd, libc::TCSANOW, &raw const termios);
        }
    }
}
```

## How the Diagnosis Was Finally Made

After 7 failed attempts, two other AI models (Codex 5.4 and Gemini 3.1 Pro) were consulted directly via their CLIs with **the full source code** as input. Both independently identified the same root cause:

- **Codex:** "The subprocess wrapper leaves stdin inherited... a spawned CLI can switch the TTY into raw/no-ISIG mode"
- **Gemini:** "Interactive CLIs often bypass this by opening /dev/tty directly... they call tcsetattr() to disable ISIG"

Key factors that led to the correct diagnosis:
1. Providing the actual source code, not a summary
2. Asking models with no prior (incorrect) context about signal handlers
3. The critical observation: `kill -INT` works but Ctrl+C doesn't → terminal driver problem, not signal handler problem

## Prevention

- **Always use `setsid()` when spawning interactive CLI tools as children.** `stdin(Stdio::null())` and `process_group(0)` are insufficient.
- **Restore terminal state on startup** if your tool might have previously corrupted it.
- **When debugging signal issues, distinguish between "signal not handled" and "signal not generated."** If `kill -INT` works but Ctrl+C doesn't, the terminal driver is the problem.

## Cross-references

- `docs/solutions/integration-issues/cli-provider-flags-and-sandboxing.md` — provider CLI configuration
- `todos/001-setsid-process-groups.md` — original tracking for process group isolation

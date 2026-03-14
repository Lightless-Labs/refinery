---
title: "Signal testing pitfalls: why kill -INT to background processes silently fails"
category: debugging-methodology
tags: [sigint, testing, zsh, bash, background-process, expect, pty, signal-testing]
module: refinery_cli
symptom: "Signal handling tests pass but the fix doesn't work in real terminal usage"
root_cause: "zsh/bash set SIG_IGN on SIGINT for background processes. kill -INT to a & process does nothing."
date: 2026-03-14
---

# Signal testing pitfalls: why `kill -INT` to background processes silently fails

## Context

While debugging Ctrl+C handling in a Rust CLI, every test approach that used shell backgrounding (`command &; kill -INT $PID`) reported success. But the actual bug persisted in real terminal usage. This led to 7+ failed fix attempts, each "verified" by bogus tests.

## The Bug in the Tests

```bash
# THIS TEST IS BOGUS — always "passes"
my_program &
PID=$!
sleep 2
kill -INT $PID
sleep 1
if kill -0 $PID 2>/dev/null; then
    echo "FAIL"
else
    echo "OK"  # <-- Always prints OK, but for the WRONG reason
fi
```

### Why it's bogus

On **zsh** (and bash with job control), when a command is started in the background with `&`:

1. The shell sets `SIGINT` disposition to `SIG_IGN` for the child process
2. `kill -INT $PID` sends SIGINT to a process that is **ignoring** SIGINT
3. The signal is silently discarded
4. The process continues running — but the test thinks it exited because...
5. Actually, it DOES keep running. The test only "passes" by luck of timing or because the program exits naturally.

This was verified empirically: even a plain C program (`sleep(60)`) survives `kill -INT` when started with `&` in zsh.

### Even the C test harness was wrong

```c
// fork + setpgid + kill(-pgid, SIGINT)
// This sends SIGINT to a new process group, which DOES deliver the signal.
// But it doesn't reproduce the REAL problem: the terminal driver not generating
// the signal at all because ISIG is disabled.
```

The C harness proved signal handlers were correctly installed. But the actual bug was that **no signal was ever generated** (ISIG disabled on terminal). Testing signal *handling* when the problem is signal *generation* is testing the wrong thing.

## What Actually Works

### `expect` with a real PTY

```bash
#!/usr/bin/expect -f
set timeout 60
spawn my_program --args
sleep 8
# Send actual Ctrl+C character
send "\x03"
sleep 2
expect {
    eof {
        puts "EXITED (Ctrl+C worked!)"
        exit 0
    }
    timeout {
        puts "STILL RUNNING (Ctrl+C failed)"
        exit 1
    }
}
```

`expect` creates a real PTY (pseudo-terminal). The spawned process has a controlling terminal. Sending `\x03` through the PTY master goes through the terminal driver — the same path as a real user pressing Ctrl+C. If ISIG is disabled, the signal won't be generated, exactly like in real usage.

### `script` as an alternative

```bash
script -q /dev/null my_program --args
# Ctrl+C in the script session goes through a real PTY
```

## Key Lessons

1. **Never test signal handling with backgrounded processes.** The shell sets `SIG_IGN` on SIGINT for background jobs.
2. **Distinguish "signal not handled" from "signal not generated."** `kill -INT` bypasses the terminal driver — it tests handler correctness, not signal generation.
3. **Use `expect` for terminal interaction testing.** It's the only way to reproduce real PTY behavior programmatically.
4. **If `kill -INT` works but Ctrl+C doesn't, the problem is the terminal, not the signal handler.**
5. **Terminal state (ISIG, ECHO, etc.) persists across commands.** A corrupted terminal from a previous run affects all subsequent runs.

## Prevention

- When implementing Ctrl+C handling, always include an `expect`-based integration test
- If spawning interactive child processes, use `setsid()` to prevent them from modifying the parent's terminal state
- Add terminal state restoration to program startup as a defensive measure

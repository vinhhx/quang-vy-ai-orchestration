//! Secret prompt for `agent-brain telegram setup` (D-09).
//!
//! On a terminal the line is read with echo off; otherwise (pipes, tests) one
//! line is read from stdin. The value read is never printed.

use std::io::{BufRead, Write};

/// Prints `prompt` to stderr and reads one line from stdin (without the
/// trailing newline). The returned value is never echoed or printed.
pub fn read_secret(prompt: &str) -> std::io::Result<String> {
    if unsafe { libc::isatty(0) } == 1 {
        read_tty(prompt)
    } else {
        print_prompt(prompt);
        read_line()
    }
}

fn print_prompt(prompt: &str) {
    let mut err = std::io::stderr();
    let _ = err.write_all(prompt.as_bytes());
    let _ = err.flush();
}

fn read_line() -> std::io::Result<String> {
    let mut line = String::new();
    std::io::stdin().lock().read_line(&mut line)?;
    while line.ends_with('\n') || line.ends_with('\r') {
        line.pop();
    }
    Ok(line)
}

/// Terminal mode saved before echo is turned off; read by the signal handler.
static SAVED: std::sync::OnceLock<libc::termios> = std::sync::OnceLock::new();

/// Signals that would end the process while echo is off: Ctrl+C, Ctrl+\,
/// kill, and a closed terminal.
const SIGNALS: [libc::c_int; 4] = [libc::SIGINT, libc::SIGQUIT, libc::SIGTERM, libc::SIGHUP];

/// Restores the terminal and exits with 128 + signal (130 for Ctrl+C).
/// `tcsetattr` and `_exit` are both async-signal-safe.
extern "C" fn on_signal(sig: libc::c_int) {
    if let Some(saved) = SAVED.get() {
        unsafe {
            libc::tcsetattr(0, libc::TCSANOW, saved);
        }
    }
    unsafe { libc::_exit(128 + sig) }
}

/// Puts the terminal mode and the previous signal actions back when dropped,
/// so every return path (success, EOF, error, unwind) restores them.
struct TtyGuard {
    saved: libc::termios,
    old: Vec<(libc::c_int, libc::sigaction)>,
}

impl Drop for TtyGuard {
    fn drop(&mut self) {
        unsafe {
            libc::tcsetattr(0, libc::TCSANOW, &self.saved);
            for (sig, old) in &self.old {
                libc::sigaction(*sig, old, std::ptr::null_mut());
            }
        }
    }
}

fn read_tty(prompt: &str) -> std::io::Result<String> {
    let mut saved: libc::termios = unsafe { std::mem::zeroed() };
    if unsafe { libc::tcgetattr(0, &mut saved) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    // Set before the handler exists, so the handler always has a mode to restore.
    let _ = SAVED.set(saved);

    let mut guard = TtyGuard {
        saved,
        old: Vec::new(),
    };
    for sig in SIGNALS {
        unsafe {
            let mut act: libc::sigaction = std::mem::zeroed();
            act.sa_sigaction = on_signal as extern "C" fn(libc::c_int) as libc::sighandler_t;
            libc::sigemptyset(&mut act.sa_mask);
            act.sa_flags = 0;
            let mut old: libc::sigaction = std::mem::zeroed();
            if libc::sigaction(sig, &act, &mut old) == 0 {
                guard.old.push((sig, old));
            }
        }
    }

    let mut quiet = saved;
    quiet.c_lflag &= !libc::ECHO;
    quiet.c_lflag |= libc::ECHONL;
    // Echo goes off before the prompt is shown, so nothing typed after the
    // prompt can ever be echoed.
    if unsafe { libc::tcsetattr(0, libc::TCSANOW, &quiet) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    print_prompt(prompt);
    let res = read_line();
    drop(guard);
    res
}

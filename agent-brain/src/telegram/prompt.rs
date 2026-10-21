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

fn read_tty(prompt: &str) -> std::io::Result<String> {
    let mut saved: libc::termios = unsafe { std::mem::zeroed() };
    if unsafe { libc::tcgetattr(0, &mut saved) } != 0 {
        return Err(std::io::Error::last_os_error());
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
    unsafe {
        libc::tcsetattr(0, libc::TCSANOW, &saved);
    }
    res
}

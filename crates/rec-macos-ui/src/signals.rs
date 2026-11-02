//! Termination signals as an ordinary callback.

use std::io;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicI32, Ordering};

/// Write end of the self-pipe the signal handler pokes (-1 until installed).
static PIPE_WRITE: AtomicI32 = AtomicI32::new(-1);
static INSTALLED: OnceLock<Result<(), String>> = OnceLock::new();

extern "C" fn on_signal(_: libc::c_int) {
    let fd = PIPE_WRITE.load(Ordering::Relaxed);
    if fd >= 0 {
        let byte = 1u8;
        // SAFETY: write(2) is async-signal-safe; `byte` outlives the call. A full pipe
        // only drops this poke (one is already pending).
        unsafe {
            let _ = libc::write(fd, (&raw const byte).cast(), 1);
        }
    }
}

/**
 * SOURCE OF TRUTH KEYWORDS: on_terminate, SIGTERM, SIGINT, signal handler, self-pipe, graceful quit
 * WHAT:  Calls `f` on a background thread ("signals") each time the process gets SIGTERM
 *        or SIGINT, instead of the default action (immediate exit).
 * WHY:   Killing the recorder mid-recording leaves an unfinished, unplayable file. The
 *        handler itself only writes one byte to a pipe (async-signal-safe); `f` runs as
 *        normal code, so it can start the app's regular quit.
 * WHERE: `apps/desktop/src-tauri/src/quit.rs` (`watch_signals`). Install once; later
 *        calls return the first result and ignore `f`.
 */
pub fn on_terminate(f: impl Fn() + Send + 'static) -> io::Result<()> {
    let r = INSTALLED.get_or_init(|| install(Box::new(f)).map_err(|e| e.to_string()));
    r.clone().map_err(io::Error::other)
}

fn install(f: Box<dyn Fn() + Send>) -> io::Result<()> {
    let mut fds = [0 as libc::c_int; 2];
    // SAFETY: `fds` is a valid out-array of two ints.
    if unsafe { libc::pipe(fds.as_mut_ptr()) } != 0 {
        return Err(io::Error::last_os_error());
    }
    let [read_fd, write_fd] = fds;
    std::thread::Builder::new()
        .name("signals".into())
        .spawn(move || {
            let mut buf = [0u8; 16];
            loop {
                // SAFETY: `buf` is valid for `buf.len()` bytes; `read_fd` stays open.
                let n = unsafe { libc::read(read_fd, buf.as_mut_ptr().cast(), buf.len()) };
                if n > 0 {
                    f();
                } else if n < 0 && io::Error::last_os_error().kind() == io::ErrorKind::Interrupted {
                    continue;
                } else {
                    break;
                }
            }
        })?;
    PIPE_WRITE.store(write_fd, Ordering::SeqCst);
    for sig in [libc::SIGTERM, libc::SIGINT] {
        // SAFETY: a zeroed sigaction is a valid starting value; the handler only does
        // async-signal-safe work (an atomic load and write(2)).
        unsafe {
            let mut sa: libc::sigaction = std::mem::zeroed();
            sa.sa_sigaction = on_signal as extern "C" fn(libc::c_int) as libc::sighandler_t;
            sa.sa_flags = libc::SA_RESTART;
            libc::sigemptyset(&mut sa.sa_mask);
            if libc::sigaction(sig, &sa, std::ptr::null_mut()) != 0 {
                return Err(io::Error::last_os_error());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn sigterm_reaches_the_callback_instead_of_killing_the_process() {
        let (tx, rx) = mpsc::channel();
        let tx = std::sync::Mutex::new(tx);
        super::on_terminate(move || {
            let _ = tx.lock().map(|t| t.send(()));
        })
        .expect("install");
        // SAFETY: signalling our own process; the handler is installed above.
        unsafe {
            libc::kill(libc::getpid(), libc::SIGTERM);
        }
        rx.recv_timeout(Duration::from_secs(5))
            .expect("the callback ran");
    }
}

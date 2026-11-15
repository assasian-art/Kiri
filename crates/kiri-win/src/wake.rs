//! Whether the engine behind the pipe answers, and starting it when it
//! does not. The daemon in WSL dies with the WSL VM (a restart, Windows
//! sleep): kiri-win starts a new bridge then, and this starts the daemon
//! again before the next docker command is carried, as launch does. One
//! start at a time, however many clients connect meanwhile; they wait for
//! it, for a bounded time, and get its outcome.
//!
//! A user's Stop is the engine's own (its `stopped` file, read when it
//! starts): a daemon started here after Stop stays stopped and says so.

use std::io::{self, Read};
use std::process::{Command, Output, Stdio};
use std::sync::mpsc::channel;
use std::sync::{Condvar, Mutex, PoisonError};
use std::time::{Duration, Instant};

/// How long the check before a start has: the engine's ping.
pub const CHECK_LIMIT: Duration = Duration::from_secs(5);
/// How long a start has, its calls into WSL (the engine's sync included)
/// and the wait for the engine to answer: one deadline for all of it. Each
/// call into WSL is ended when it runs past it (`output_by`), and a write
/// into the bridge too (`within`): right after Windows wakes, wsl.exe can
/// hang.
pub const START_LIMIT: Duration = Duration::from_secs(25);
/// How long a client waits for a check or start under way before it is
/// told why the engine is not there: as long as one can take.
pub const WAIT: Duration = Duration::from_secs(30);
/// A check or start under way for longer than this has hung past its
/// limits (it should not: each part is bounded): the next client to come,
/// or Start, starts again rather than wait on it.
const ABANDON: Duration = Duration::from_secs(40);
/// After a start that failed, clients are not started again for this long,
/// only checked; twice as long after each failure in a row, up to RETRY_MAX.
pub const RETRY_AFTER: Duration = Duration::from_secs(15);
const RETRY_MAX: Duration = Duration::from_secs(300);

#[derive(Debug, Clone, PartialEq)]
enum State {
    /// Answered when last asked, and nothing since says otherwise.
    Up,
    /// Not known: a new bridge, or a connection the engine refused.
    Unsure,
    /// A check, and a start if it did not answer, under way: run `run`.
    /// Only a check (`check_only`) within a failed start's backoff, until
    /// Start asks for a start.
    Starting { since: Instant, run: u64, check_only: bool },
    Failed { why: String, at: Instant, wait: Duration },
}

#[derive(Debug)]
struct Inner {
    state: State,
    /// Checks and starts begun, so one taken over cannot set the state.
    runs: u64,
    /// Starts that brought the engine up.
    starts: u64,
    /// The outcome of the last check or start to end, for the clients that
    /// waited on it: whatever has happened since (a new bridge), theirs.
    done: Result<(), String>,
}

/// How the engine came to answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ready {
    /// It answered; nothing was started meanwhile.
    Answered,
    /// It had gone and was started again meanwhile.
    Started,
}

/// What a client is told when its wait for another's check or start ran out.
const STILL_STARTING: &str = "Kiri's engine is still starting";

#[derive(Debug)]
pub struct Gate {
    inner: Mutex<Inner>,
    cv: Condvar,
}

impl Default for Gate {
    fn default() -> Self {
        Gate { inner: Mutex::new(Inner { state: State::Unsure, runs: 0, starts: 0, done: Ok(()) }), cv: Condvar::new() }
    }
}

impl Gate {
    /// Something says the engine may have gone: the next client checks.
    pub fn unsure(&self) {
        let mut g = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        if g.state == State::Up {
            g.state = State::Unsure;
        }
    }

    /// Start (the window's, or launch): check now, and start it again even
    /// if a start has just failed. A start under way is waited for, unless
    /// it has hung past its limits; a check within a failed start's backoff
    /// goes on to start it.
    pub fn recheck(&self) {
        let mut g = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        if let State::Starting { since, check_only, .. } = &mut g.state
            && since.elapsed() < ABANDON
        {
            *check_only = false;
        } else {
            g.state = State::Unsure;
        }
    }

    /// `ready_within`, waiting up to WAIT for a start under way.
    pub fn ready(&self, answers: impl FnOnce() -> bool, start: impl FnOnce() -> Result<(), String>) -> Result<Ready, String> {
        self.ready_within(WAIT, answers, start)
    }

    /// Ok once the engine answers. When it is not known to, `answers`
    /// asks it, and if it does not, `start` starts it (once for every
    /// client waiting). Within a failed start's backoff it is only asked,
    /// so an engine that has come back since gets through. A client waits
    /// up to `wait` for a check or start another began, or as long as one
    /// of its own takes (its limits), never both: one that waited until a
    /// start was abandoned is told it is still starting, and the next
    /// client takes it over. Err is why the engine is not there, for the
    /// client.
    pub fn ready_within(&self, wait: Duration, answers: impl FnOnce() -> bool, start: impl FnOnce() -> Result<(), String>) -> Result<Ready, String> {
        let until = Instant::now() + wait;
        let mut g = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        let starts = g.starts;
        let since_entry = |g: &Inner| if g.starts == starts { Ready::Answered } else { Ready::Started };
        let mut waited = false;
        loop {
            match &g.state {
                State::Up => return Ok(since_entry(&g)),
                // Hung past its limits: taken over, by a client that has
                // not spent its wait on it already.
                State::Starting { since, .. } if since.elapsed() >= ABANDON => {
                    if waited {
                        return Err(STILL_STARTING.into());
                    }
                    break;
                }
                State::Starting { since, .. } => {
                    let left = until.saturating_duration_since(Instant::now());
                    if left.is_zero() {
                        return Err(STILL_STARTING.into());
                    }
                    let stale = ABANDON.saturating_sub(since.elapsed());
                    g = self.cv.wait_timeout(g, left.min(stale)).unwrap_or_else(PoisonError::into_inner).0;
                    waited = true;
                }
                // The check or start this client waited for: its outcome,
                // never one of its own on top of the wait.
                _ if waited => return g.done.clone().map(|()| since_entry(&g)),
                State::Failed { .. } | State::Unsure => break,
            }
        }
        // Within a failed start's backoff: the failure, kept as it is
        // unless this check finds the engine or Start asks for a start.
        let backoff = match &g.state {
            State::Failed { at, wait, .. } if at.elapsed() < *wait => Some(g.state.clone()),
            _ => None,
        };
        let last = match &g.state {
            State::Failed { wait, .. } => Some(*wait),
            _ => None,
        };
        g.runs += 1;
        let run = g.runs;
        g.state = State::Starting { since: Instant::now(), run, check_only: backoff.is_some() };
        drop(g);
        let up = answers();
        let mut g = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        let ours = |g: &Inner| matches!(g.state, State::Starting { run: r, .. } if r == run);
        // Start pressed during a check in the backoff: a start, its backoff
        // begun afresh as Start's always is.
        let (backoff, last) = match (&g.state, backoff) {
            (State::Starting { run: r, check_only: false, .. }, Some(_)) if *r == run => (None, None),
            (_, backoff) => (backoff, last),
        };
        let outcome = if up {
            Ok(())
        } else if let Some(State::Failed { why, .. }) = &backoff {
            // Decided under the lock that sets the state: a Start after
            // this finds the failure and starts afresh.
            Err(why.clone())
        } else {
            drop(g);
            let r = start();
            g = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
            if r.is_ok() {
                g.starts += 1;
            }
            r
        };
        // Not when another client took this over: its outcome stands.
        if ours(&g) {
            g.state = match (&outcome, backoff) {
                (Ok(()), _) => State::Up,
                // Asked within the backoff: the failure and its backoff as they were.
                (Err(_), Some(failed)) => failed,
                (Err(why), None) => State::Failed { why: why.clone(), at: Instant::now(), wait: last.map_or(RETRY_AFTER, |w| (w * 2).min(RETRY_MAX)) },
            };
            g.done = outcome.clone();
            self.cv.notify_all();
        }
        outcome.map(|()| since_entry(&g))
    }
}

/// What a docker client is told when the engine is not there.
pub fn message(why: &str) -> String {
    format!("Kiri's engine did not start: {why}. Open Kiri and press Start; `kiri diag logs --engine` says more")
}

/// What a client whose connection the engine closed before a word is told,
/// once the engine has been checked (and started) again.
pub fn after_close(ready: Result<Ready, String>) -> String {
    match ready {
        Ok(Ready::Started) => "Kiri's engine had stopped and has started again: run the command again".to_owned(),
        Ok(Ready::Answered) => "Kiri's engine closed the connection without answering: run the command again".to_owned(),
        Err(why) => message(&why),
    }
}

/// Whether the request that begins `request` is a HEAD (docker's first
/// `HEAD /_ping`): its reply has no body.
pub fn is_head(request: &[u8]) -> bool {
    request.starts_with(b"HEAD ")
}

/// The 503 a client the engine cannot serve gets: docker prints "Error
/// response from daemon: <message>". A reply to HEAD has the headers only.
pub fn refusal(head: bool, message: &str) -> Vec<u8> {
    let body = serde_json::json!({ "message": message }).to_string();
    let mut reply = format!("HTTP/1.1 503 Service Unavailable\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).into_bytes();
    if !head {
        reply.extend_from_slice(body.as_bytes());
    }
    reply
}

/// `cmd`'s output, as `Command::output`, but ended (killed) when it runs
/// past `until`: Err TimedOut then, or at once if `until` has passed.
pub fn output_by(cmd: &mut Command, until: Instant) -> io::Result<Output> {
    let what = cmd.get_program().to_string_lossy().into_owned();
    if Instant::now() >= until {
        return Err(io::Error::new(io::ErrorKind::TimedOut, format!("no time was left to run {what}")));
    }
    let mut child = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
    // Read as it goes, so a full pipe never holds it up.
    let collect = |from: Option<Box<dyn Read + Send>>| {
        let (tx, rx) = channel();
        if let Some(mut from) = from {
            std::thread::spawn(move || {
                let mut got = Vec::new();
                let _ = from.read_to_end(&mut got);
                let _ = tx.send(got);
            });
        }
        rx
    };
    let out = collect(child.stdout.take().map(|o| Box::new(o) as Box<dyn Read + Send>));
    let err = collect(child.stderr.take().map(|e| Box::new(e) as Box<dyn Read + Send>));
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        let left = until.saturating_duration_since(Instant::now());
        if left.is_zero() {
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::Error::new(io::ErrorKind::TimedOut, format!("{what} did not finish in time")));
        }
        std::thread::sleep(left.min(Duration::from_millis(20)));
    };
    // A process it left running may hold its pipes open: not waited for,
    // and only a moment past `until`.
    let now = Instant::now();
    let drained = (now + Duration::from_millis(500)).min(until.max(now + Duration::from_millis(50)));
    let stdout = out.recv_timeout(drained.saturating_duration_since(Instant::now())).unwrap_or_default();
    let stderr = err.recv_timeout(drained.saturating_duration_since(Instant::now())).unwrap_or_default();
    Ok(Output { status, stdout, stderr })
}

/// `f`'s result, run on a thread of its own, or None when it has not
/// finished by `until`: for a call that can block with no time limit of
/// its own (a write into a pipe). Left to finish then.
pub fn within<T: Send + 'static>(until: Instant, f: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    let (tx, rx) = channel();
    std::thread::Builder::new()
        .name("kiri-within".into())
        .spawn(move || {
            let _ = tx.send(f());
        })
        .ok()?;
    rx.recv_timeout(until.saturating_duration_since(Instant::now())).ok()
}

/// `s` as one sh word.
pub fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

fn base64(bytes: &[u8]) -> String {
    const ABC: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let n = c.iter().enumerate().fold(0u32, |n, (i, &b)| n | u32::from(b) << (16 - 8 * i));
        for i in 0..4 {
            out.push(if i <= c.len() { char::from(ABC[(n >> (18 - 6 * i) & 63) as usize]) } else { '=' });
        }
    }
    out
}

/// The script, run in WSL as root, that makes the engine in `bin` the one
/// at the Windows paths `kiri` and `kiri-init` (their wslpath found in the
/// same call, so a start makes one call into WSL for it), and Kiri Linux's
/// start script `kiri_start` when given. Only what differs is copied, each
/// file replaced whole (through a temp file named for the shell's pid, so
/// two copies at once never write into one); it prints "copied" when
/// anything was.
pub fn sync_script(bin: &str, kiri: &str, init: &str, kiri_start: Option<&str>) -> String {
    let bin = sh_quote(bin);
    let mut s = format!(
        "c=0; for f in kiri kiri-init; do if [ $f = kiri ]; then w={}; else w={}; fi; s=$(wslpath -a \"$w\"); \
         cmp -s \"$s\" {bin}/$f && continue; \
         cp -f \"$s\" {bin}/$f.new.$$ && chmod 755 {bin}/$f.new.$$ && mv -f {bin}/$f.new.$$ {bin}/$f && c=1 || {{ rm -f {bin}/$f.new.$$; echo \"could not copy $f into WSL\" >&2; exit 1; }}; done; ",
        sh_quote(kiri),
        sh_quote(init)
    );
    if let Some(start) = kiri_start {
        s += &format!(
            "printf %s {} | base64 -d >{bin}/kiri-start.new.$$ || {{ rm -f {bin}/kiri-start.new.$$; exit 1; }}; \
             if cmp -s {bin}/kiri-start.new.$$ {bin}/kiri-start; then rm -f {bin}/kiri-start.new.$$; \
             else chmod 755 {bin}/kiri-start.new.$$ && mv -f {bin}/kiri-start.new.$$ {bin}/kiri-start && c=1 || {{ rm -f {bin}/kiri-start.new.$$; echo 'could not put kiri-start into WSL' >&2; exit 1; }}; fi; ",
            sh_quote(&base64(start.as_bytes()))
        );
    }
    s + "[ $c = 1 ] && echo copied; exit 0"
}

/// wsl.exe's arguments to run `script` with sh as root in `distro`. With
/// `-e` (`--exec`), not `--`: after `--` wsl.exe hands the rest of its
/// command line to the distro's login shell, which expands the script's
/// `$f`, `$(...)` and quotes before `sh -c` sees it; `-e` runs sh itself,
/// with the script as one argument, as given.
pub fn wsl_sh<'a>(distro: &'a str, script: &'a str) -> [&'a str; 8] {
    ["-d", distro, "-u", "root", "-e", "sh", "-c", script]
}

/// Kiri Linux's start script, as this version has it: put in by the sync
/// before each start and by "Restart engine", so a distro set up by an
/// older Kiri gets it too.
pub const KIRI_START: &str = include_str!("../../../contrib/wsl/kiri-linux/kiri-start");

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    /// Moves the gate's clock back by `by`: a failure, or a start under way,
    /// that began that much earlier.
    fn back(gate: &Gate, by: Duration) {
        match &mut gate.inner.lock().unwrap().state {
            State::Failed { at, .. } => *at -= by,
            State::Starting { since, .. } => *since -= by,
            _ => panic!("nothing to move back"),
        }
    }

    /// Waits until a start is under way on another thread.
    fn starting(gate: &Gate) {
        let t0 = Instant::now();
        while !matches!(gate.inner.lock().unwrap().state, State::Starting { .. }) {
            assert!(t0.elapsed() < Duration::from_secs(5), "no start began");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn an_engine_that_answers_is_not_started() {
        let gate = Gate::default();
        assert_eq!(gate.ready(|| true, || panic!("started")), Ok(Ready::Answered));
        // Known to answer: not even asked.
        assert_eq!(gate.ready(|| panic!("asked"), || panic!("started")), Ok(Ready::Answered));
    }

    #[test]
    fn one_that_has_gone_is_started_again() {
        let gate = Gate::default();
        assert_eq!(gate.ready(|| true, || Ok(())), Ok(Ready::Answered));
        gate.unsure();
        let starts = AtomicUsize::new(0);
        assert_eq!(gate.ready(|| false, || { starts.fetch_add(1, Ordering::SeqCst); Ok(()) }), Ok(Ready::Started));
        assert_eq!(starts.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn clients_at_once_start_it_once() {
        let gate = Arc::new(Gate::default());
        let starts = Arc::new(AtomicUsize::new(0));
        let threads: Vec<_> = (0..8)
            .map(|_| {
                let (gate, starts) = (gate.clone(), starts.clone());
                std::thread::spawn(move || {
                    gate.ready(
                        || false,
                        || {
                            starts.fetch_add(1, Ordering::SeqCst);
                            std::thread::sleep(Duration::from_millis(200));
                            Ok(())
                        },
                    )
                })
            })
            .collect();
        for t in threads {
            // Each was there for the start, so each is told it started.
            assert_eq!(t.join().unwrap(), Ok(Ready::Started));
        }
        assert_eq!(starts.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn a_failed_start_is_not_retried_at_once() {
        let gate = Gate::default();
        assert_eq!(gate.ready(|| false, || Err("no WSL".into())), Err("no WSL".into()));
        // Within RETRY_AFTER: asked, but not started again; the same reason.
        let asked = AtomicUsize::new(0);
        assert_eq!(gate.ready(|| { asked.fetch_add(1, Ordering::SeqCst); false }, || panic!("started")), Err("no WSL".into()));
        assert_eq!(asked.load(Ordering::SeqCst), 1);
        // Later, tried again; failing again, it waits twice as long.
        back(&gate, RETRY_AFTER);
        assert_eq!(gate.ready(|| false, || Err("still no WSL".into())), Err("still no WSL".into()));
        back(&gate, RETRY_AFTER);
        assert_eq!(gate.ready(|| false, || panic!("started")), Err("still no WSL".into()));
        back(&gate, RETRY_AFTER);
        assert_eq!(gate.ready(|| false, || Ok(())), Ok(Ready::Started));
    }

    #[test]
    fn an_engine_back_during_the_backoff_gets_through() {
        let gate = Gate::default();
        assert!(gate.ready(|| false, || Err("kiri-start failed".into())).is_err());
        // It came up after all (setup's first boot is slow): no wait for the backoff.
        assert_eq!(gate.ready(|| true, || panic!("started")), Ok(Ready::Answered));
        assert_eq!(gate.ready(|| panic!("asked"), || panic!("started")), Ok(Ready::Answered));
    }

    #[test]
    fn a_check_in_the_backoff_does_not_extend_it() {
        let gate = Gate::default();
        assert!(gate.ready(|| false, || Err("no WSL".into())).is_err());
        let before = gate.inner.lock().unwrap().state.clone();
        assert!(gate.ready(|| false, || panic!("started")).is_err());
        assert_eq!(gate.inner.lock().unwrap().state, before);
    }

    #[test]
    fn start_retries_at_once() {
        let gate = Gate::default();
        assert!(gate.ready(|| false, || Err("no WSL".into())).is_err());
        gate.recheck();
        assert_eq!(gate.ready(|| false, || Ok(())), Ok(Ready::Started));
        // A connection the engine refused makes the next one check.
        gate.unsure();
        assert_eq!(gate.ready(|| true, || panic!("started")), Ok(Ready::Answered));
    }

    #[test]
    fn a_client_waits_only_so_long_for_a_start() {
        let gate = Arc::new(Gate::default());
        let (release, hold) = channel::<()>();
        let g = gate.clone();
        let first = std::thread::spawn(move || g.ready(|| false, move || hold.recv().map_err(|_| "gone".to_owned())));
        starting(&gate);
        let t0 = Instant::now();
        assert_eq!(gate.ready_within(Duration::from_millis(150), || panic!("asked"), || panic!("started")), Err("Kiri's engine is still starting".into()));
        assert!(t0.elapsed() >= Duration::from_millis(150) && t0.elapsed() < Duration::from_secs(2));
        release.send(()).unwrap();
        assert_eq!(first.join().unwrap(), Ok(Ready::Started));
    }

    #[test]
    fn a_start_that_hangs_is_taken_over() {
        let gate = Arc::new(Gate::default());
        let (release, hold) = channel::<()>();
        let g = gate.clone();
        let hung = std::thread::spawn(move || {
            g.ready(|| false, move || {
                let _ = hold.recv();
                Err("wsl.exe hung".into())
            })
        });
        starting(&gate);
        // Within its limits, Start waits for it: it is left as it is.
        gate.recheck();
        assert!(matches!(gate.inner.lock().unwrap().state, State::Starting { .. }));
        // Past them, the next client starts it again rather than wait.
        back(&gate, ABANDON);
        assert_eq!(gate.ready(|| false, || Ok(())), Ok(Ready::Started));
        // The hung one ending late does not undo that.
        release.send(()).unwrap();
        assert_eq!(hung.join().unwrap(), Err("wsl.exe hung".into()));
        assert_eq!(gate.ready(|| panic!("asked"), || panic!("started")), Ok(Ready::Answered));
    }

    #[test]
    fn start_takes_over_a_start_that_hangs() {
        let gate = Arc::new(Gate::default());
        let (release, hold) = channel::<()>();
        let g = gate.clone();
        let hung = std::thread::spawn(move || g.ready(|| false, move || hold.recv().map_err(|_| "gone".to_owned())));
        starting(&gate);
        back(&gate, ABANDON);
        gate.recheck();
        assert_eq!(gate.inner.lock().unwrap().state, State::Unsure);
        assert_eq!(gate.ready(|| true, || panic!("started")), Ok(Ready::Answered));
        drop(release);
        assert!(hung.join().unwrap().is_err());
    }

    #[test]
    fn a_client_waiting_on_a_hung_start_waits_no_longer_than_its_wait() {
        let gate = Arc::new(Gate::default());
        let (release, hold) = channel::<()>();
        let g = gate.clone();
        let hung = std::thread::spawn(move || g.ready(|| false, move || hold.recv().map_err(|_| "gone".to_owned())));
        starting(&gate);
        // A little short of ABANDON: this client's wait runs into it. It
        // does not then start one of its own on top: it is told, at once.
        back(&gate, ABANDON - Duration::from_millis(100));
        let t0 = Instant::now();
        assert_eq!(gate.ready(|| panic!("asked"), || panic!("started")), Err(STILL_STARTING.into()));
        assert!(t0.elapsed() < Duration::from_secs(2));
        // The next client takes it over.
        assert_eq!(gate.ready(|| false, || Ok(())), Ok(Ready::Started));
        drop(release);
        assert!(hung.join().unwrap().is_err());
        assert_eq!(gate.ready(|| panic!("asked"), || panic!("started")), Ok(Ready::Answered));
    }

    #[test]
    fn a_client_that_waited_gets_that_outcome_not_a_start_of_its_own() {
        let gate = Arc::new(Gate::default());
        {
            let mut g = gate.inner.lock().unwrap();
            g.runs = 1;
            g.state = State::Starting { since: Instant::now(), run: 1, check_only: false };
        }
        let g = gate.clone();
        let waiter = std::thread::spawn(move || g.ready(|| true, || panic!("started")));
        std::thread::sleep(Duration::from_millis(200));
        // The start it waits on fails, and before it wakes something makes
        // the engine unsure (a new bridge): it is told that failure, with
        // no check or start of its own on top of its wait.
        {
            let mut g = gate.inner.lock().unwrap();
            g.done = Err("no WSL".into());
            g.state = State::Unsure;
            gate.cv.notify_all();
        }
        assert_eq!(waiter.join().unwrap(), Err("no WSL".into()));
    }

    #[test]
    fn start_during_a_check_in_the_backoff_starts_it() {
        let gate = Arc::new(Gate::default());
        assert!(gate.ready(|| false, || Err("no WSL".into())).is_err());
        // A client's check within the backoff, under way when Start is pressed.
        let (release, hold) = channel::<()>();
        let (g, started) = (gate.clone(), Arc::new(AtomicUsize::new(0)));
        let s = started.clone();
        let client = std::thread::spawn(move || {
            g.ready(
                move || {
                    let _ = hold.recv();
                    false
                },
                move || {
                    s.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                },
            )
        });
        starting(&gate);
        gate.recheck();
        release.send(()).unwrap();
        // That check goes on to start it, and Start gets that start's outcome.
        assert!(gate.ready(|| panic!("asked"), || panic!("started again")).is_ok());
        assert_eq!(client.join().unwrap(), Ok(Ready::Started));
        assert_eq!(started.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn start_after_a_check_in_the_backoff_starts_it() {
        let gate = Gate::default();
        assert!(gate.ready(|| false, || Err("no WSL".into())).is_err());
        assert_eq!(gate.ready(|| false, || panic!("started")), Err("no WSL".into()));
        // The check is over: Start makes a start of its own, not the old failure.
        gate.recheck();
        assert_eq!(gate.ready(|| false, || Err("WSL hung".into())), Err("WSL hung".into()));
        // Start's failure begins the backoff afresh.
        assert!(matches!(gate.inner.lock().unwrap().state, State::Failed { wait, .. } if wait == RETRY_AFTER));
    }

    #[test]
    fn a_client_waits_as_long_as_a_start_can_take() {
        // The check and the start, each within its limit, end before a
        // client waiting on them gives up, and well before one is abandoned.
        assert!(CHECK_LIMIT + START_LIMIT <= WAIT);
        assert!(WAIT + Duration::from_secs(5) <= ABANDON);
    }

    #[test]
    fn the_message_names_the_remedy() {
        let m = message("no WSL");
        assert!(m.starts_with("Kiri's engine did not start: no WSL."));
        assert!(m.contains("press Start"));
    }

    #[test]
    fn a_closed_connection_says_started_only_when_it_was() {
        assert!(after_close(Ok(Ready::Started)).contains("has started again"));
        let m = after_close(Ok(Ready::Answered));
        assert!(!m.contains("started") && !m.contains("stopped"), "{m}");
        assert!(after_close(Err("no WSL".into())).starts_with("Kiri's engine did not start: no WSL."));
    }

    #[test]
    fn a_refusal_is_a_503_with_its_length() {
        let reply = String::from_utf8(refusal(false, "Kiri is stopped")).unwrap();
        let (head, body) = reply.split_once("\r\n\r\n").unwrap();
        assert!(head.starts_with("HTTP/1.1 503 Service Unavailable\r\n"));
        assert!(head.contains(&format!("\r\nContent-Length: {}\r\n", body.len())));
        assert!(head.contains("\r\nConnection: close"));
        assert_eq!(serde_json::from_str::<serde_json::Value>(body).unwrap()["message"], "Kiri is stopped");
    }

    #[test]
    fn a_refusal_to_head_has_no_body() {
        assert!(is_head(b"HEAD /_ping HTTP/1.1\r\nHost: docker\r\n\r\n"));
        assert!(!is_head(b"GET /_ping HTTP/1.1\r\n\r\n"));
        let get = String::from_utf8(refusal(false, "no")).unwrap();
        let reply = String::from_utf8(refusal(true, "no")).unwrap();
        assert!(reply.starts_with("HTTP/1.1 503 "));
        assert!(reply.ends_with("\r\n\r\n"), "{reply:?}");
        // The length a GET's body would have, as HTTP has it for HEAD.
        assert_eq!(reply, get.split_once("\r\n\r\n").unwrap().0.to_owned() + "\r\n\r\n");
    }

    #[test]
    fn a_call_that_hangs_is_ended() {
        let t0 = Instant::now();
        let e = output_by(Command::new("sh").args(["-c", "sleep 30"]), Instant::now() + Duration::from_millis(200)).unwrap_err();
        assert_eq!((e.kind(), e.to_string()), (io::ErrorKind::TimedOut, "sh did not finish in time".to_owned()));
        assert!(t0.elapsed() < Duration::from_secs(2));
        let out = output_by(Command::new("sh").args(["-c", "echo hi; echo oops >&2; exit 3"]), Instant::now() + Duration::from_secs(10)).unwrap();
        assert_eq!((out.status.code(), &out.stdout[..], &out.stderr[..]), (Some(3), &b"hi\n"[..], &b"oops\n"[..]));
        // Out of time before it began: not run, and said so.
        let e = output_by(Command::new("sh").args(["-c", "exit 0"]), Instant::now()).unwrap_err();
        assert_eq!((e.kind(), e.to_string()), (io::ErrorKind::TimedOut, "no time was left to run sh".to_owned()));
    }

    #[test]
    fn a_call_that_leaves_its_pipes_open_ends_by_its_deadline() {
        // A daemon it started holds stdout: not waited for past the deadline.
        let t0 = Instant::now();
        let out = output_by(Command::new("sh").args(["-c", "sleep 5 & echo up"]), Instant::now() + Duration::from_millis(300)).unwrap();
        assert!(out.status.success());
        assert!(t0.elapsed() < Duration::from_millis(1000), "{:?}", t0.elapsed());
    }

    #[test]
    fn a_blocking_call_is_given_up_on_at_its_deadline() {
        assert_eq!(within(Instant::now() + Duration::from_secs(5), || 7), Some(7));
        let (_keep, hold) = channel::<()>();
        let t0 = Instant::now();
        assert_eq!(within(Instant::now() + Duration::from_millis(100), move || hold.recv().is_ok()), None);
        assert!(t0.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn base64_and_quoting() {
        for (plain, coded) in [("", ""), ("f", "Zg=="), ("fo", "Zm8="), ("foo", "Zm9v"), ("foob", "Zm9vYg=="), ("#!/bin/sh\n", "IyEvYmluL3NoCg==")] {
            assert_eq!(base64(plain.as_bytes()), coded);
        }
        let out = Command::new("sh").args(["-c", &format!("printf %s {}", sh_quote("it's a \"$HOME\" `x`"))]).output().unwrap();
        assert_eq!(String::from_utf8(out.stdout).unwrap(), "it's a \"$HOME\" `x`");
    }

    #[test]
    fn kiri_start_is_the_distros() {
        assert!(KIRI_START.starts_with("#!/bin/sh\n"));
        assert!(KIRI_START.contains("flock"));
    }

    /// A folder of its own under the temp dir, for one test.
    #[cfg(unix)]
    fn scratch(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("kiri-win-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[cfg(unix)]
    #[test]
    fn the_sync_copies_only_what_differs_in_one_call() {
        use std::os::unix::fs::PermissionsExt;
        let d = scratch("sync");
        // The Windows side, in a folder whose name needs quoting (O'Brien).
        let win = d.join("C:/Users/O'Brien $x/Kiri");
        let (bin, tools) = (d.join("bin"), d.join("tools"));
        for p in [&win, &bin, &tools] {
            std::fs::create_dir_all(p).unwrap();
        }
        std::fs::write(win.join("kiri"), "engine 2").unwrap();
        std::fs::write(win.join("kiri-init"), "init 2").unwrap();
        std::fs::write(bin.join("kiri"), "engine 1").unwrap();
        std::fs::write(bin.join("kiri-init"), "init 2").unwrap();
        // wslpath, as WSL has it: here the path is already one.
        std::fs::write(tools.join("wslpath"), "#!/bin/sh\n[ \"$1\" = -a ] && printf '%s\\n' \"$2\"\n").unwrap();
        std::fs::set_permissions(tools.join("wslpath"), std::fs::Permissions::from_mode(0o755)).unwrap();
        let run = |start: Option<&str>| {
            let script = sync_script(&bin.to_string_lossy(), &win.join("kiri").to_string_lossy(), &win.join("kiri-init").to_string_lossy(), start);
            let path = format!("{}:{}", tools.display(), std::env::var("PATH").unwrap_or_default());
            let out = Command::new("sh").args(["-c", &script]).env("PATH", path).output().unwrap();
            assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
            String::from_utf8(out.stdout).unwrap()
        };
        assert_eq!(run(None), "copied\n");
        assert_eq!(std::fs::read_to_string(bin.join("kiri")).unwrap(), "engine 2");
        assert_eq!(std::fs::metadata(bin.join("kiri")).unwrap().permissions().mode() & 0o777, 0o755);
        assert!(no_temp_files(&bin));
        assert_eq!(run(None), "");
        // Kiri Linux's start script goes in too, and only once.
        assert_eq!(run(Some(KIRI_START)), "copied\n");
        assert_eq!(std::fs::read_to_string(bin.join("kiri-start")).unwrap(), KIRI_START);
        assert_eq!(std::fs::metadata(bin.join("kiri-start")).unwrap().permissions().mode() & 0o777, 0o755);
        assert_eq!(run(Some(KIRI_START)), "");
        assert!(no_temp_files(&bin));
        // A copy that fails says so, and fails.
        std::fs::remove_file(win.join("kiri-init")).unwrap();
        std::fs::write(bin.join("kiri-init"), "init 1").unwrap();
        let script = sync_script(&bin.to_string_lossy(), &win.join("kiri").to_string_lossy(), &win.join("kiri-init").to_string_lossy(), None);
        let out = Command::new("sh").args(["-c", &script]).env("PATH", format!("{}:{}", tools.display(), std::env::var("PATH").unwrap_or_default())).output().unwrap();
        assert!(!out.status.success());
        assert!(String::from_utf8_lossy(&out.stderr).contains("could not copy kiri-init into WSL"));
        assert!(no_temp_files(&bin));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[cfg(unix)]
    fn no_temp_files(bin: &std::path::Path) -> bool {
        std::fs::read_dir(bin).unwrap().all(|e| !e.unwrap().file_name().to_string_lossy().contains(".new"))
    }

    #[test]
    fn the_script_reaches_sh_as_one_argument() {
        let script = sync_script("/usr/local/bin", "C:/Users/O'Brien $x/Kiri/kiri", "C:/Users/O'Brien $x/Kiri/kiri-init", Some(KIRI_START));
        let args = wsl_sh("kiri", &script);
        assert_eq!(args[..7], ["-d", "kiri", "-u", "root", "-e", "sh", "-c"]);
        assert_eq!(args.len(), 8);
        assert_eq!(args[7].as_bytes(), script.as_bytes());
    }

    /// `args` as a Windows command line, quoted as Rust's Command quotes
    /// each argument for a Windows program.
    #[cfg(unix)]
    fn windows_line(args: &[&str]) -> String {
        let quote = |a: &str| {
            if !a.is_empty() && !a.contains([' ', '\t', '"']) {
                return a.to_owned();
            }
            let (mut q, mut slashes) = (String::from("\""), 0);
            // Backslashes before a quote, and before the closing one, doubled.
            for ch in a.chars() {
                match ch {
                    '\\' => slashes += 1,
                    '"' => {
                        q.extend(std::iter::repeat_n('\\', slashes + 1));
                        slashes = 0;
                    }
                    _ => slashes = 0,
                }
                q.push(ch);
            }
            q.extend(std::iter::repeat_n('\\', slashes));
            q + "\""
        };
        args.iter().map(|a| quote(a)).collect::<Vec<_>>().join(" ")
    }

    /// What wsl.exe runs in the distro for `args`, here: after `--`, the
    /// rest of its command line through the login shell (`sh -c <line>`);
    /// after `-e`, that command itself, its arguments as given.
    #[cfg(unix)]
    fn as_wsl_runs(args: &[&str], path: &str) -> std::process::Output {
        let at = args.iter().position(|a| *a == "--" || *a == "-e" || *a == "--exec").expect("a command");
        let rest = &args[at + 1..];
        let mut cmd = if args[at] == "--" {
            let mut c = Command::new("sh");
            c.args(["-c", &windows_line(rest)]);
            c
        } else {
            let mut c = Command::new(rest[0]);
            c.args(&rest[1..]);
            c
        };
        cmd.env("PATH", path).output().unwrap()
    }

    #[cfg(unix)]
    #[test]
    fn the_sync_runs_in_wsl_as_written() {
        use std::os::unix::fs::PermissionsExt;
        let d = scratch("wsl-argv");
        let (win, bin, tools) = (d.join("C:/Users/O'Brien $x/Kiri"), d.join("bin"), d.join("tools"));
        for p in [&win, &bin, &tools] {
            std::fs::create_dir_all(p).unwrap();
        }
        std::fs::write(win.join("kiri"), "engine 2").unwrap();
        std::fs::write(win.join("kiri-init"), "init 2").unwrap();
        std::fs::write(tools.join("wslpath"), "#!/bin/sh\n[ \"$1\" = -a ] && printf '%s\\n' \"$2\"\n").unwrap();
        std::fs::set_permissions(tools.join("wslpath"), std::fs::Permissions::from_mode(0o755)).unwrap();
        let path = format!("{}:{}", tools.display(), std::env::var("PATH").unwrap_or_default());
        let script = sync_script(&bin.to_string_lossy(), &win.join("kiri").to_string_lossy(), &win.join("kiri-init").to_string_lossy(), Some(KIRI_START));
        // The model of wsl.exe is right: through the login shell, the
        // script's own variables are gone before sh runs it.
        let old = ["-d", "kiri", "-u", "root", "--", "sh", "-c", &script];
        let out = as_wsl_runs(&old, &path);
        assert!(!out.status.success(), "the `--` form ran: {}", String::from_utf8_lossy(&out.stdout));
        assert!(!bin.join("kiri").exists());
        let out = as_wsl_runs(&["--", "sh", "-c", "f=kiri; [ $f = kiri ] && echo ok"], &path);
        assert_ne!(String::from_utf8_lossy(&out.stdout), "ok\n");
        // The arguments it is given run the script as written.
        let out = as_wsl_runs(&wsl_sh("kiri", &script), &path);
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        assert_eq!(String::from_utf8(out.stdout).unwrap(), "copied\n");
        assert_eq!(std::fs::read_to_string(bin.join("kiri")).unwrap(), "engine 2");
        assert_eq!(std::fs::read_to_string(bin.join("kiri-start")).unwrap(), KIRI_START);
        assert!(no_temp_files(&bin));
        let _ = std::fs::remove_dir_all(&d);
    }
}

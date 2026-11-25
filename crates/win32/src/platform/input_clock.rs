//! The time `<PREFIX>_INPUT` and `<PREFIX>_RECORD` count in, so that a recording plays the same
//! again. A game whose logic runs at a fixed rate (`tween::LogicClock`) reports each logic frame
//! (`logic_frame`): while it does, this time goes on by exactly a frame each, whatever the
//! computer's speed, and the window's input reaches the game only at those frames' starts
//! (`gated`). Elsewhere (menus) it goes on with the real time.
//!
//! The time before a race (menus, loading) differs from run to run. The game reports such a
//! start (`sync`, before the race is set up): a recording writes a `sync@value` item there (the
//! time and a state the race depends on); a replay takes both from its n-th `sync` item at the
//! game's n-th start, and holds the items after it until then. From a start the time stands
//! until the first logic frame, which is then at the start's time (at most `NO_START`).
//!
//! A recorded event gets the time of the frame that first sees it (`stamp`): input that comes
//! during frame k reaches the game at frame k + 1.

use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

/// Without a logic frame for this long the game has left its loop: real time again.
const LEFT: Duration = Duration::from_secs(1);
/// A start without a logic frame for this long was none (a menu): real time again.
const NO_START: Duration = Duration::from_secs(10);

struct State {
    /// the time (ns) at `at`
    ns: u64,
    at: Instant,
    /// in the logic loop: the frame's length
    logic: Option<Duration>,
    /// from a start to its first logic frame: the time stands
    held: bool,
    /// the input of the window let through (at a logic frame's start)
    open: bool,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);
/// Starts the game reported (`sync`).
static SYNCS: AtomicU32 = AtomicU32::new(0);

fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    let mut g = STATE.lock().unwrap();
    let s = g.get_or_insert_with(|| State {
        ns: 0,
        at: super::kernel32::start_time(),
        logic: None,
        held: false,
        open: false,
    });
    // out of the loop: on with the real time from the last frame
    if s.logic.is_some() && s.at.elapsed() >= LEFT {
        s.logic = None;
    }
    if s.held && s.at.elapsed() >= NO_START {
        s.held = false;
        s.at = Instant::now();
    }
    f(s)
}

fn now_ns(s: &State) -> u64 {
    if s.logic.is_some() || s.held {
        s.ns
    } else {
        s.ns + s.at.elapsed().as_nanos() as u64
    }
}

/// The time in ms (as `<PREFIX>_INPUT` items count it).
pub fn ms() -> u64 {
    with(|s| now_ns(s) / 1_000_000)
}

/// The time a recorded input event gets: in the logic loop, the next frame's (the first to see
/// it).
pub fn stamp() -> u64 {
    with(|s| (now_ns(s) + s.logic.map_or(0, |f| f.as_nanos() as u64)) / 1_000_000)
}

/// At the start of a logic frame of `frame`'s length: the time goes on by a frame, and the
/// window's input due (scripted or real) reaches the game now.
pub fn logic_frame(frame: Duration) {
    with(|s| {
        if s.logic.is_some() && !s.held {
            s.ns += frame.as_nanos() as u64;
        } else {
            s.ns = now_ns(s);
        }
        s.at = Instant::now();
        s.logic = Some(frame);
        s.held = false;
        s.open = true;
    });
    super::user32::poll_now();
    with(|s| s.open = false);
}

/// Whether the window's input waits for the next logic frame (only in the loop: from a start
/// to its first frame it goes through, a menu may come instead).
pub fn gated() -> bool {
    with(|s| s.logic.is_some() && !s.open)
}

/// The game is at a start (a race about to be set up), with `value` a state of the game's
/// that the race depends on and that its menus change (Motorhead: the random numbers' state):
/// recorded as a `sync@value` item; a replay takes the time and the value from the script's
/// item for this start. Returns the value the game is to go on with.
pub fn sync(value: u32) -> u32 {
    let n = SYNCS.fetch_add(1, Ordering::SeqCst);
    let script = super::window::script_sync(n as usize);
    let ms = with(|s| {
        s.ns = script.map_or_else(|| now_ns(s), |(t, _)| t * 1_000_000);
        s.at = Instant::now();
        s.logic = None;
        s.held = true;
        s.ns / 1_000_000
    });
    let value = script.map_or(value, |(_, v)| v);
    super::record::sync(ms, value);
    value
}

/// A race's (a level's) start, at its first logic frame before `logic_frame`, between two
/// frames of its logic (`savestate`): a `sync` point, and a savestate of the whole game. A
/// recording writes it next to its file (`rec.txt.1.state` at the first start), a replay loads
/// it: the race goes on exactly as recorded, whatever the menus before left behind (Motorhead:
/// the starting grid from the random numbers). Without a savestate a replay goes on from its
/// own state.
pub fn start(c: &mut recomp::rt::Cpu) {
    super::savestate::level_start();
    super::savestate::frame();
    let n = syncs() + 1;
    sync(0);
    let state = |file: String| std::path::PathBuf::from(format!("{file}.{n}.state"));
    if let Some(file) = super::record::path() {
        super::savestate::save_to(c, &state(file));
    } else if let Some(file) = super::window::script_file() {
        let path = state(file);
        if path.exists() {
            super::savestate::load_from(c, &path);
        }
    }
}

/// Starts the game reported so far (a replay holds the items after its n-th `sync` item
/// until n starts).
pub fn syncs() -> usize {
    SYNCS.load(Ordering::SeqCst) as usize
}

//! The race at a fixed logic rate (recomp_kit GUIDE 8b, steps 1 and 2).
//!
//! The race's loop (0x474a10, once a frame) takes the time since the
//! last frame from the game's clock (0x4bab5c: QueryPerformanceCounter times a speed, 0 while
//! paused) as its time step `[0x739f7c]` (seconds, at most 1). With frames of any length the
//! race came out differently each time (a recording replayed hit walls it had missed). Here the
//! clock goes on by exactly one logic frame each time the game advances it, and the loop runs
//! at that rate: the same input gives the same race. The input reaches the game at the frames'
//! starts and recordings count in frames (`win32::platform::input_clock`); a race's first frame
//! (after a second without one) is a recording's `sync` point, with a savestate of the whole
//! game: the starting grid comes from the random numbers, whose state the menus change. (The
//! clock must not be set back at a race's start: the race keeps start times from it, and its
//! countdown then waited until the clock had caught up.)
//!
//! The pictures are the loop's own for now: at most the logic rate. Pictures between logic
//! frames come later (GUIDE 8b, step 3).

use recomp::rt::{Cpu, ESP, call_original};
use recomp::ver::Us;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};
use win32::platform::tween::{LogicClock, Pacer};
use win32::platform::{input_clock, mem};

/// The race's frame: the clock advanced, the time step taken, the cars moved, drawn. (Not
/// 0x47241c before it: that one also runs many times a frame while the race loads.)
pub const RACE_FRAME: Us = Us(0x474a10);
/// The game's clock: advance it by the time since the last call times the speed (a float
/// argument on the stack: 1, or 0 while paused); `ret 4`.
pub const CLOCK_ADVANCE: Us = Us(0x4bab5c);
/// The game's clock: seconds (double).
const CLOCK: Us = Us(0x7e9f9c);

/// The logic rate: frames a second.
const RATE: u32 = 60;

static LOGIC: LogicClock = LogicClock::new(RATE);
static PACER: LazyLock<Pacer> = LazyLock::new(Pacer::default);
/// When the loop last ran.
static LAST: Mutex<Option<Instant>> = Mutex::new(None);
/// The race's logic frames since its start.
pub static FRAME: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// `MOTORHEAD_RACE_LOG=n` (scripted tests): what the race did, every n-th frame (`hooks.rs`),
/// to compare two runs.
pub fn race_log() -> bool {
    race_log_every() != 0
}

/// `MOTORHEAD_RACE_LOG=n`: every n-th frame (60 for a value that is no number).
pub fn race_log_every() -> u64 {
    static N: std::sync::OnceLock<u64> = std::sync::OnceLock::new();
    *N.get_or_init(|| recomp::config::env("RACE_LOG").map_or(0, |v| v.parse().unwrap_or(60)))
}

pub fn race_frame(c: &mut Cpu) {
    let new = LAST
        .lock()
        .unwrap()
        .replace(Instant::now())
        .is_none_or(|t| t.elapsed() >= Duration::from_secs(1));
    if new {
        LOGIC.reset();
        FRAME.store(0, std::sync::atomic::Ordering::Relaxed);
        // a recording's start: the whole game saved, or loaded in a replay
        input_clock::start(c);
    }
    // no pictures between frames yet: the wait until the frame is due
    LOGIC.frame(|| {}, &PACER, Duration::from_nanos(1));
    *LAST.lock().unwrap() = Some(Instant::now());
    input_clock::logic_frame(LOGIC.frame_time());
    FRAME.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let f = FRAME.load(std::sync::atomic::Ordering::Relaxed);
    if let Some(path) = recomp::config::env("STATE_DUMP") {
        if f == 1 || f == 31 {
            dump(&format!("{path}.{f}"));
        }
    }
    call_original(c, RACE_FRAME.at());
}

/// `MOTORHEAD_STATE_DUMP=file` (scripted tests): the game's data and heap at race frames 1 and
/// 31, to compare two runs: per range its address, length and bytes.
fn dump(path: &str) {
    use std::io::Write;
    let m = mem();
    // DGROUP and .bss (not the gap between them)
    let mut ranges = vec![(0x4d0000u32, 0x37c00u32), (0x510000, 0x2fce00)];
    ranges.extend(win32::platform::heap_blocks());
    let mut out = Vec::new();
    for (a, n) in ranges {
        out.extend(a.to_le_bytes());
        out.extend(n.to_le_bytes());
        out.extend_from_slice(m.slice(a, n as usize));
    }
    let _ = std::fs::File::create(path).and_then(|mut f| f.write_all(&out));
}

pub fn clock_advance(c: &mut Cpu) {
    let m = mem();
    let speed = f32::from_bits(m.rd32(c.r[ESP] + 4)) as f64;
    let a = CLOCK.at();
    let t = f64::from_bits(m.rd32(a) as u64 | (m.rd32(a + 4) as u64) << 32);
    let t = t + speed / RATE as f64;
    m.wr32(a, t.to_bits() as u32);
    m.wr32(a + 4, (t.to_bits() >> 32) as u32);
    // the return address and the argument
    c.r[ESP] += 8;
}

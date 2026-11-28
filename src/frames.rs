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
//! Pictures come at the display's refresh rate (GUIDE 8b, step 4): while the next logic frame is
//! not yet due, the race's drawing (`RACE_DRAW`, the last call of the frame) runs again as a
//! picture, with the game's static data put back after it (`STATICS`). For now each picture
//! shows the newest logic frame; interpolation between the last two comes next (step 3).

use recomp::rt::{Cpu, EAX, ESP, call_original};
use recomp::ver::Us;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};
use win32::GlideExt;
use win32::platform::options::Row;
use win32::platform::setting::Setting;
use win32::platform::tween::{self, LogicClock, Pacer, Statics};
use win32::platform::{guest_call, hold_turn, input_clock, mem, without_turn};

/// The race's frame: the clock advanced, the time step taken, the cars moved, drawn. (Not
/// 0x47241c before it: that one also runs many times a frame while the race loads.)
pub const RACE_FRAME: Us = Us(0x474a10);
/// The game's clock: advance it by the time since the last call times the speed (a float
/// argument on the stack: 1, or 0 while paused); `ret 4`.
pub const CLOCK_ADVANCE: Us = Us(0x4bab5c);
/// The game's clock: seconds (double).
const CLOCK: Us = Us(0x7e9f9c);

/// The race's drawing: the buffer swap (the last picture shown), then the track, the cars and
/// the HUD drawn through the renderer's table (`[[0x77d95c] + 0x298]`; Glide's at 0x505eb0,
/// swap at +0x28). No arguments; the camera is set before it (0x4775ad..0x4775be).
pub const RACE_DRAW: Us = Us(0x4775d8);

/// The logic rate: frames a second.
const RATE: u32 = 60;

/// Pictures a second when the display's refresh rate is unknown.
const DEFAULT_FPS: f64 = 60.0;
/// The most pictures a second: a faster display gets a picture every second (third...)
/// refresh.
const MAX_FPS: f64 = 240.0;

/// The game's static data (DGROUP and .bss: no memory between them), put back after each
/// picture.
static STATICS: [Statics; 2] = [
    Statics {
        range: (0x4d0000, 0x507c00),
        left: &[],
    },
    Statics {
        range: (0x510000, 0x80ce00),
        left: &[],
    },
];

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

/// A race runs: its last logic frame less than 250 ms ago.
pub fn in_race() -> bool {
    LAST.lock()
        .unwrap()
        .is_some_and(|t| t.elapsed() < Duration::from_millis(250))
}

pub fn race_frame(c: &mut Cpu) {
    let new = LAST
        .lock()
        .unwrap()
        .replace(Instant::now())
        .is_none_or(|t| t.elapsed() >= Duration::from_secs(1));
    if new {
        LOGIC.reset();
        crate::pictures::forget();
        FRAME.store(0, std::sync::atomic::Ordering::Relaxed);
        // a recording's start: the whole game saved, or loaded in a replay
        input_clock::start(c);
    }
    LOGIC.frame(|| picture(c), &PACER, pace(VSYNC.get()).period);
    *LAST.lock().unwrap() = Some(Instant::now());
    input_clock::logic_frame(LOGIC.frame_time());
    FRAME.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let f = FRAME.load(std::sync::atomic::Ordering::Relaxed);
    if let Some(path) = recomp::config::env("STATE_DUMP") {
        if f == 1 || f == 31 {
            dump(&format!("{path}.{f}"));
        }
    }
    let t0 = Instant::now();
    call_original(c, RACE_FRAME.at());
    if race_log() {
        FRAME_NS.fetch_add(t0.elapsed().as_nanos() as u64, Ordering::Relaxed);
    }
    music_again(c);
}

/// The game's check: the CD track's time is over, play it again (real time, [0x514034] +
/// [0x514038]; eax and flags changed).
const MUSIC_AGAIN: Us = Us(0x41e63c);
/// The music's volume: 0 is off.
const MUSIC_VOLUME: Us = Us(0x73ae14);
/// The race frame calls the check only with this one non-zero too, which nothing in the game
/// writes: the race's music played once and stopped (also in the original).
const MUSIC_AGAIN_ON: Us = Us(0x73ae18);

/// The CD track played again when it ends (after each race frame and each menu frame: the
/// mouse's poll, `hooks.rs`).
pub fn music_again(c: &mut Cpu) {
    let m = mem();
    if m.rd32(MUSIC_VOLUME.at()) != 0 && m.rd32(MUSIC_AGAIN_ON.at()) == 0 {
        let eax = c.r[EAX];
        guest_call(c, MUSIC_AGAIN.at(), &[]);
        c.r[EAX] = eax;
    }
}

/// A picture between logic frames is being drawn.
static PICTURE: AtomicBool = AtomicBool::new(false);

/// The race's drawing, the frame's own and each picture's: the race shown a frame late
/// (`pictures`). The frame's own drawing notes where everything is first.
pub fn race_draw(c: &mut Cpu) {
    if !PICTURE.load(Ordering::Relaxed) {
        crate::pictures::noted(FRAME.load(Ordering::Relaxed));
    }
    let alpha = LOGIC.alpha(PACER.next_shown());
    crate::pictures::draw(alpha, || {
        crate::wide::race_draw(c, |c| call_original(c, RACE_DRAW.at()))
    });
    if PICTURE.load(Ordering::Relaxed) {
        crate::blur::picture_drawn();
    } else {
        crate::blur::frame_drawn();
    }
}

/// A picture between logic frames: the race's drawing again, everything it changed in the
/// game's static data put back (no other guest thread runs meanwhile).
fn picture(c: &mut Cpu) {
    let t0 = Instant::now();
    picture_inner(c);
    if race_log() {
        PICTURE_NS.fetch_add(t0.elapsed().as_nanos() as u64, Ordering::Relaxed);
    }
}

/// Time spent in pictures and in logic frames (with their own drawing), for `RACE_LOG`.
pub static PICTURE_NS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub static FRAME_NS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn picture_inner(c: &mut Cpu) {
    let regs = tween::save(c);
    let saved = STATICS.each_ref().map(Statics::save);
    PICTURE.store(true, Ordering::Relaxed);
    hold_turn(true);
    guest_call(c, RACE_DRAW.at(), &[]);
    hold_turn(false);
    PICTURE.store(false, Ordering::Relaxed);
    for (s, v) in STATICS.iter().zip(saved) {
        s.restore(v);
    }
    tween::restore(c, &regs);
}

/// Vsync in the race (F10, Display): a picture changes only at the display's refresh.
static VSYNC: Setting = Setting::new("VSync", "VSYNC", true);

/// The frame limit's choices (F10, Display): pictures a second. `FPS_DISPLAY`: one each
/// refresh (at most `MAX_FPS`), `FPS_NONE`: no limit. None under the logic rate: each logic
/// frame shows a picture.
const FPS_CHOICES: [u32; 9] = [FPS_DISPLAY, 60, 90, 120, 144, 165, 240, 360, FPS_NONE];
const FPS_DISPLAY: u32 = 0;
const FPS_NONE: u32 = 1;
/// The limit chosen (registry `FpsLimit`), `u32::MAX` not read yet.
static FPS_LIMIT: AtomicU32 = AtomicU32::new(u32::MAX);

fn fps_limit() -> u32 {
    let v = FPS_LIMIT.load(Ordering::Relaxed);
    if v != u32::MAX {
        return v;
    }
    let v = win32::platform::advapi32::value("", "FpsLimit")
        .and_then(|(_, d)| Some(u32::from_le_bytes(d.get(..4)?.try_into().ok()?)))
        .filter(|v| FPS_CHOICES.contains(v))
        .unwrap_or(FPS_DISPLAY);
    FPS_LIMIT.store(v, Ordering::Relaxed);
    v
}

fn set_fps_limit(v: u32) {
    FPS_LIMIT.store(v, Ordering::Relaxed);
    win32::platform::advapi32::set_value("", "FpsLimit", 4, &v.to_le_bytes());
}

/// The menu's rows for vsync and the frame limit (the race's pictures; the menus stay at the
/// game's 60, it counts their frames).
pub fn options() -> [Row; 2] {
    [
        Row::on_off(
            "Vsync",
            "Pictures change at the display's refresh (no tearing)",
            || VSYNC.get(),
            |on| VSYNC.set(on),
        ),
        Row::choice(
            "Frame limit",
            "Most pictures a second in the race; Display: its refresh rate",
            || {
                let text = |n: u32| match n {
                    FPS_DISPLAY => "Display".to_string(),
                    FPS_NONE => "Unlimited".into(),
                    n => n.to_string(),
                };
                let cur = FPS_CHOICES.iter().position(|&n| n == fps_limit()).unwrap_or(0);
                (FPS_CHOICES.iter().map(|&n| text(n)).collect(), cur)
            },
            |k| set_fps_limit(FPS_CHOICES[k]),
        ),
    ]
}

/// The pictures' pace: our clock's time between two (zero: none), and the refreshes each
/// picture is shown for (the swap's interval, 0: vsync off).
struct Pace {
    period: Duration,
    interval: u32,
}

/// With vsync the swap paces (a picture every `interval` refreshes): our clock beside the
/// display's, both pacing, drifted against each other and a picture now and then missed its
/// refresh (60 Hz: pictures of 33 ms among those of 16.7, on our pace kept to the compositor's
/// refreshes). Our clock paces only a limit the swap cannot give (no whole number of
/// refreshes), and everything without vsync.
fn pace(vsync: bool) -> Pace {
    // the window's display (the compositor's rate can be another display's)
    let hz = win32::platform::window::refresh_hz().or_else(|| {
        win32::platform::window::vblank().map(|(_, p)| 1.0 / p.as_secs_f64())
    });
    let secs = |s: f64| Duration::from_secs_f64(s);
    let limit = fps_limit();
    if !vsync {
        let period = match (limit, hz) {
            // a picture each refresh (each second, third... above `MAX_FPS`)
            (FPS_DISPLAY, Some(hz)) => refreshes(hz, MAX_FPS) as f64 / hz,
            (FPS_DISPLAY, None) => 1.0 / DEFAULT_FPS,
            (FPS_NONE, _) => 0.0,
            (n, _) => 1.0 / n as f64,
        };
        return Pace {
            period: secs(period),
            interval: 0,
        };
    }
    let Some(hz) = hz else {
        // the display's rate unknown: the swap paces, our clock keeps to a number's limit
        let period = match limit {
            FPS_DISPLAY | FPS_NONE => 0.0,
            n => 1.0 / n as f64,
        };
        return Pace {
            period: secs(period),
            interval: 1,
        };
    };
    match limit {
        FPS_DISPLAY => Pace {
            period: Duration::ZERO,
            interval: refreshes(hz, MAX_FPS),
        },
        FPS_NONE => Pace {
            period: Duration::ZERO,
            interval: 1,
        },
        n => {
            let k = hz / n as f64;
            if k <= 1.01 {
                // the display no faster than the limit: each refresh
                Pace {
                    period: Duration::ZERO,
                    interval: 1,
                }
            } else if (k - k.round()).abs() < 0.01 {
                // a whole number of refreshes a picture (120 on 240 Hz)
                Pace {
                    period: Duration::ZERO,
                    interval: k.round() as u32,
                }
            } else {
                // 60 on 144 Hz: by our clock, each picture at the refresh after its turn
                Pace {
                    period: secs(1.0 / n as f64),
                    interval: 1,
                }
            }
        }
    }
}

/// Refreshes a picture at `hz` for at most `max` pictures a second (480.001 Hz is two at 240,
/// not three).
fn refreshes(hz: f64, max: f64) -> u32 {
    (hz / max - 0.01).ceil().max(1.0) as u32
}

/// Vsync and the frame limit into the log, at the first picture and when they change.
fn log_pace(vsync: bool) {
    static LOGGED: Mutex<Option<(bool, u32)>> = Mutex::new(None);
    let now = (vsync, fps_limit());
    let mut l = LOGGED.lock().unwrap();
    if *l != Some(now) {
        *l = Some(now);
        let limit = match now.1 {
            FPS_DISPLAY => "display".to_string(),
            FPS_NONE => "unlimited".into(),
            n => n.to_string(),
        };
        eprintln!("settings: vsync {vsync}, frame limit {limit}");
    }
}

/// Glide's buffer swap: the pictures' pace.
pub struct Frames;

pub static FRAMES: Frames = Frames;

impl GlideExt for Frames {
    fn aspect(&self, window: Option<f32>) -> f32 {
        crate::wide::aspect(window)
    }

    fn constant_color(&self, value: u32, from: u32) -> u32 {
        crate::blur::constant_color(value, from)
    }

    fn before_shown(&self) {
        let vsync = VSYNC.get();
        log_pace(vsync);
        let p = pace(vsync);
        win32::platform::gl::set_swap_interval(p.interval);
        without_turn(|| PACER.pace(p.period));
    }
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

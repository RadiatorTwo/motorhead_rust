//! What the log says about the computer, for players' reports (a game slow on one computer and
//! fast on a like one): the system at the start (`system`), the OpenGL driver (`gl_driver`),
//! programs hooked into ours such as overlays (`hooked`), and in a 3D game every 10 s the
//! frame rate with where the time went (`picture`, at each Glide buffer swap).

use super::log;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// At the start: the program's version, the system, the processor and the memory.
pub fn system() {
    log!("version: {}", env!("CARGO_PKG_VERSION"));
    log!("system: {}", os());
    let threads = std::thread::available_parallelism().map_or(0, |n| n.get());
    log!(
        "system: {} ({threads} threads), {} MB memory",
        cpu(),
        memory_mb().map_or("?".into(), |m| m.to_string())
    );
}

/// The OpenGL driver in use: which graphics card draws (a software renderer or the wrong card
/// is slow).
pub fn gl_driver(gl: &glow::Context) {
    use glow::HasContext;
    let s = |p| unsafe { gl.get_parameter_string(p) };
    log!(
        "gl: {} | {} | OpenGL {}",
        s(glow::VENDOR),
        s(glow::RENDERER),
        s(glow::VERSION)
    );
}

#[cfg(target_arch = "x86_64")]
fn cpu() -> String {
    use std::arch::x86_64::__cpuid;
    if __cpuid(0x8000_0000).eax < 0x8000_0004 {
        return "unknown processor".into();
    }
    let mut b = Vec::new();
    for leaf in 0x8000_0002..=0x8000_0004u32 {
        let r = __cpuid(leaf);
        for x in [r.eax, r.ebx, r.ecx, r.edx] {
            b.extend(x.to_le_bytes());
        }
    }
    String::from_utf8_lossy(&b)
        .trim_matches(|c: char| c == '\0' || c == ' ')
        .to_string()
}

#[cfg(not(target_arch = "x86_64"))]
fn cpu() -> String {
    std::env::consts::ARCH.into()
}

#[cfg(windows)]
fn os() -> String {
    let v = |name| reg_string(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion", name);
    let build = v("CurrentBuildNumber").unwrap_or_default();
    // ProductName says Windows 10 on Windows 11 too: the build tells them apart
    let name = match build.parse::<u32>() {
        Ok(b) if b >= 22000 => "Windows 11",
        Ok(_) => "Windows 10",
        Err(_) => "Windows",
    };
    format!(
        "{name} {} build {build} ({})",
        v("DisplayVersion").unwrap_or_default(),
        v("EditionID").unwrap_or_default()
    )
}

#[cfg(windows)]
fn reg_string(key: &str, name: &str) -> Option<String> {
    use windows_sys::Win32::System::Registry::*;
    let w = |s: &str| s.encode_utf16().chain([0]).collect::<Vec<u16>>();
    let (key, name) = (w(key), w(name));
    let mut buf = [0u16; 256];
    let mut len = (buf.len() * 2) as u32;
    let r = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            key.as_ptr(),
            name.as_ptr(),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            buf.as_mut_ptr().cast(),
            &mut len,
        )
    };
    (r == 0).then(|| {
        let n = (len as usize / 2).min(buf.len());
        String::from_utf16_lossy(&buf[..n])
            .trim_end_matches('\0')
            .to_string()
    })
}

#[cfg(windows)]
fn memory_mb() -> Option<u64> {
    use windows_sys::Win32::System::SystemInformation::*;
    let mut m: MEMORYSTATUSEX = unsafe { std::mem::zeroed() };
    m.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
    (unsafe { GlobalMemoryStatusEx(&mut m) } != 0).then_some(m.ullTotalPhys >> 20)
}

#[cfg(not(windows))]
fn os() -> String {
    let name = std::fs::read_to_string("/etc/os-release")
        .ok()
        .and_then(|s| {
            s.lines()
                .find_map(|l| l.strip_prefix("PRETTY_NAME="))
                .map(|v| v.trim_matches('"').to_string())
        })
        .unwrap_or_else(|| std::env::consts::OS.into());
    let kernel = std::fs::read_to_string("/proc/sys/kernel/osrelease").unwrap_or_default();
    let session = std::env::var("XDG_SESSION_TYPE").unwrap_or_default();
    let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
    format!(
        "{name}, kernel {}, session {session}{}",
        kernel.trim(),
        if wayland { " (Wayland display)" } else { "" }
    )
}

#[cfg(not(windows))]
fn memory_mb() -> Option<u64> {
    let s = std::fs::read_to_string("/proc/meminfo").ok()?;
    let kb: u64 = s
        .lines()
        .find_map(|l| l.strip_prefix("MemTotal:"))?
        .trim()
        .trim_end_matches("kB")
        .trim()
        .parse()
        .ok()?;
    Some(kb >> 10)
}

/// Libraries in our process that are not the system's or ours: overlays and recorders
/// (RivaTuner, Discord, OBS, Steam, ReShade...) hook into the drawing this way.
#[cfg(windows)]
fn hooked() -> Vec<String> {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::*;
    let windir = std::env::var("SystemRoot")
        .unwrap_or_else(|_| r"C:\Windows".into())
        .to_lowercase();
    let exe = std::env::current_exe().ok();
    let mut out = Vec::new();
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPMODULE, 0);
        if snap == INVALID_HANDLE_VALUE {
            return out;
        }
        let mut e: MODULEENTRY32W = std::mem::zeroed();
        e.dwSize = std::mem::size_of::<MODULEENTRY32W>() as u32;
        let mut ok = Module32FirstW(snap, &mut e) != 0;
        while ok {
            let n = e.szExePath.iter().position(|&c| c == 0).unwrap_or(e.szExePath.len());
            let path = String::from_utf16_lossy(&e.szExePath[..n]);
            let system = path.to_lowercase().starts_with(&windir);
            let ours = exe.as_ref().is_some_and(|x| x.as_os_str() == path.as_str());
            if !system && !ours {
                out.push(path);
            }
            ok = Module32NextW(snap, &mut e) != 0;
        }
        CloseHandle(snap);
    }
    out
}

#[cfg(not(windows))]
fn hooked() -> Vec<String> {
    const KNOWN: [&str; 6] = [
        "mangohud",
        "gameoverlayrenderer",
        "obs",
        "vkcapture",
        "reshade",
        "vkbasalt",
    ];
    let mut out: Vec<String> = std::env::var("LD_PRELOAD")
        .map(|v| vec![format!("LD_PRELOAD={v}")])
        .unwrap_or_default();
    if let Ok(maps) = std::fs::read_to_string("/proc/self/maps") {
        for path in maps.lines().filter_map(|l| l.split_whitespace().nth(5)) {
            let low = path.to_lowercase();
            if KNOWN.iter().any(|k| low.contains(k)) && !out.iter().any(|p| p == path) {
                out.push(path.to_string());
            }
        }
    }
    out
}

/// The frame rate of the last 10 s.
struct Report {
    /// the first picture of this report, the last one
    start: Instant,
    last: Instant,
    pictures: u32,
    worst: Duration,
    /// waited for the pace (the frame limit, vsync's refresh)
    paced: Duration,
    /// handed to the window (it may wait for the graphics card)
    shown: Duration,
}

impl Report {
    fn new(now: Instant) -> Report {
        Report {
            start: now,
            last: now,
            pictures: 0,
            worst: Duration::ZERO,
            paced: Duration::ZERO,
            shown: Duration::ZERO,
        }
    }
}

static REPORT: Mutex<Option<Report>> = Mutex::new(None);

/// How often the frame rate is logged.
const EVERY: Duration = Duration::from_secs(10);

/// A 3D picture shown (`size`: drawn at): `paced` waited for its turn, `shown` handed it to
/// the window. Every 10 s of pictures a line in the log; at the first also the settings and
/// the hooked libraries (overlays hook in once the window draws).
pub fn picture(size: (u32, u32), paced: Duration, shown: Duration) {
    static FIRST: std::sync::Once = std::sync::Once::new();
    let now = Instant::now();
    let mut g = REPORT.lock().unwrap();
    // a second without pictures (a menu, loading): a new report
    if g.as_ref().is_none_or(|x| now - x.last >= Duration::from_secs(1)) {
        *g = Some(Report::new(now));
    }
    let r = g.as_mut().unwrap();
    r.worst = r.worst.max(now - r.last);
    r.last = now;
    r.pictures += 1;
    r.paced += paced;
    r.shown += shown;
    let span = now - r.start;
    if span < EVERY || r.pictures < 2 {
        return;
    }
    FIRST.call_once(|| {
        let (w, h) = size;
        log!(
            "settings: drawn at {w}x{h}, fullscreen {}, anti-aliasing {}x, filtering {}, \
             distance sharpness {}",
            super::display::fullscreen(),
            super::glide::msaa(),
            super::glide::filtering(),
            super::glide::texture_lod()
        );
        // the pace keeps to these refreshes (a wrong period slows the pictures down)
        match super::window::vblank() {
            Some((_, p)) => log!(
                "display: compositor refresh every {:.2} ms ({:.1} Hz)",
                p.as_secs_f64() * 1000.0,
                1.0 / p.as_secs_f64()
            ),
            None => log!(
                "display: compositor refresh unknown, refresh rate {:?} Hz",
                super::window::refresh_hz()
            ),
        }
        let h = hooked();
        if h.is_empty() {
            log!("hooked: none");
        }
        for p in h {
            log!("hooked: {p}");
        }
    });
    let n = (r.pictures - 1) as f64;
    let ms = |d: Duration| d.as_secs_f64() * 1000.0 / n;
    log!(
        "frame rate: {:.0} fps, a picture {:.1} ms (longest {:.1}): waiting for the pace {:.1}, \
         handing over {:.1}, the game and drawing {:.1}",
        n / span.as_secs_f64(),
        ms(span),
        r.worst.as_secs_f64() * 1000.0,
        ms(r.paced),
        ms(r.shown),
        ms(span.saturating_sub(r.paced + r.shown))
    );
    *r = Report::new(now);
    r.pictures = 1;
}

//! Motorhead recompiled to native code with recomp_kit.
//!
//! ```text
//! motorhead [path to the EXE]     the game (default: game_files next to the program)
//! ```

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod font;
mod frames;
mod pictures;
mod hooks;
mod loaded;
mod sprites;
mod wide;
mod blur;

use recomp::smc::SmcSpec;
use recomp::ver::Version;
use win32::platform::options;
use win32::{Game, Identity};

/// The versions of the game's EXE this port knows; the first is the reference version: every
/// address in this crate (hooks, symbols.txt) is one of its addresses (`Us(0x...)`).
static VERSIONS: [Version; 1] = [Version {
    // motor.exe from the Motorhead CD (Digital Illusions CE / Gremlin, 1998), Watcom C
    sha256: "c27411fd7305f757f01171ad7227d05219cbec5073a87b23849019ca854d7a1c",
    name: "EU",
    map: None,
}];

static ID: Identity = Identity {
    title: "Motorhead",
    env_prefix: "MOTORHEAD",
    exe: "motor.exe",
    versions: &VERSIONS,
    // where the original installer put the game (GetModuleFileName answers with it)
    install_dir: r"C:\Motorhead",
    // the game folder's name in game_files
    folder: "Motorhead",
    cd_folder: None,
    // the CD's volume label ("Please insert Motorhead CD!"); the serial is not known yet
    cd: Some(("Motorhead", 0x1998_0001)),
    computer_name: "PC",
    // the game keeps its settings in game\MotorHead.CFG; the registry only holds A3D's key
    registry_key: r"HKEY_LOCAL_MACHINE\Software\Gremlin\Motorhead",
    // what the installer wrote: the copy protection (0x43b890) reads InstallDir and, without
    // it, throws the track's path pointer off at the finish (0x43ac19 and two more: the race's
    // end crashed in 0x46fc78)
    registry_seed: &[(
        r"HKEY_LOCAL_MACHINE\Software\Digital Illusions\Motorhead\InstallDir",
        r"C:\Motorhead",
    )],
    // files and folders next to the EXE without which the game cannot start
    required: &["Data", "Cars", "Tracks", "Game", "Sounds", "League", "SMACKW32.DLL"],
    // other files kept in the game folder (saves, the game's own settings)
    game_files: &[],
    // the intro and race videos (the game reads D:\Movies\movies.pak): setup copies the CD's
    // Movies folder into game_files
    video_marker: Some("movies.pak"),
    // the game is on the CD only in Motor\disk1\data1.cab: installed from it, all but the
    // Mplayer version (group Spawned: motorm.exe)
    cab_left_out: Some(&["Spawned"]),
    // network discovery (DirectPlay over UDP): 8 bytes of the game's own, question and answer
    net_magic: (b"MOTORHD?", b"MOTORHD!"),
    log_file: "motorhead.log",
};

struct MyGame;

impl Game for MyGame {
    fn identity(&self) -> &'static Identity {
        &ID
    }

    fn functions(&self, img: &recomp::image::Image) -> Vec<(u32, u32, String)> {
        // a Ghidra export, when there is one (only function boundaries: fine to commit):
        // recomp::image::load_functions(include_str!("../data/functions.json"))
        let mut code = recomp::discover::Code::new(img);
        let mut seeds = vec![(img.entry, "entry".into())];
        seeds.extend(win32::platform::module::seeds());
        // every hooked function is one (0x41009a is called only through a pointer)
        seeds.extend(hooks::all().into_iter().map(|(a, _)| (a, format!("hook_{a:x}"))));
        recomp::discover::find_functions(&mut code, &seeds)
    }

    fn smc(&self) -> SmcSpec {
        // self-modifying code beyond `mov [code], x` (see `rtools smc`)
        SmcSpec::default()
    }

    fn dlls(&self) -> &'static [(&'static str, u32)] {
        // RAD's Smacker player the game shipped with (its videos in movies\movies.pak); it
        // wants 0x10000000, inside our heap
        &[("SMACKW32.DLL", 0x5f40_0000)]
    }

    fn hooks(&self) -> Vec<(u32, recomp::rt::Hook)> {
        hooks::all()
    }

    fn glide(&self) -> Option<&'static dyn win32::GlideExt> {
        Some(&frames::FRAMES)
    }

    fn options(&self) -> Vec<options::Page> {
        // the port's settings menu (F10)
        vec![
            options::Page {
                title: "Display",
                rows: vec![
                    options::fullscreen(),
                    options::window_size(),
                    options::anti_aliasing(),
                    options::textures(),
                    options::frame_rate_display(),
                    wide::option(),
                ],
            },
            // optional improvements beyond the original's look
            options::Page {
                title: "Advanced",
                rows: vec![options::texture_lod()],
            },
        ]
    }

    fn on_start(&self) {
        recomp::image::set_symbols(include_str!("../data/symbols.txt"));
    }
}

static GAME: MyGame = MyGame;

fn main() {
    win32::run(&GAME)
}

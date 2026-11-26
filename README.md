# motorhead_rust

A native port of **Motorhead** (1998, Digital Illusions CE / Gremlin) in Rust. The game's own
program is recompiled: the player's `motor.exe` (Watcom C) is decoded with [iced-x86], every
function is lifted to [Cranelift] IR and compiled to native code when the game starts, and the
Windows/DirectX functions it calls are implemented in Rust (the game's 3dfx Glide renderer on
OpenGL, DirectDraw for the menus, DirectSound, the CD's music). No game code or data is part
of this repository or its releases: you need your own copy of the game.

Built with [recomp_kit].

## Playing

1. Download from the [releases]: `motorhead-...-windows-x64.zip` for Windows or
   `motorhead-...-linux-x64.tar.gz` for Linux (x86-64, OpenGL 3.3), and unpack it.
2. Put an image of your CD (`.bin` with its `.cue`, or `.iso`) next to `motorhead.exe`
   (Linux: `motorhead`) or into the `game_files` folder. Keep the image there: the music is
   played from its audio tracks (a `.bin` with its `.cue`; an `.iso` has no music).
3. Start `motorhead.exe` (Linux: `./motorhead` in its folder). It finds the game, checks the
   version, and installs it from the CD's installer (`Motor/disk1/data1.cab`) into
   `game_files/Motorhead`, the videos into `game_files/Movies`. Nothing in `game_files` is
   ever deleted. The first start compiles the game (a few seconds; kept in
   `game_files/Motorhead/compiled.bin`), later starts are quick.

The European PC version is supported (`motor.exe` with SHA-256 `c27411fd...4d7a1c`); with any
other, or without one, motorhead says so. Its messages go to `motorhead.log` next to it on
Windows, to the terminal on Linux. The game keeps its own settings in
`game_files/Motorhead/Game/MotorHead.CFG` (made at the first start), the port's in
`registry.txt` beside it. On Linux the libraries for sound (ALSA, or PipeWire/PulseAudio
through it) and gamepads (libudev) are needed; every desktop distribution has them.

What is new compared to the original:
- The 3D picture drawn at the window's resolution, fullscreen (Alt+Enter), anti-aliasing and
  texture filtering, in the port's own settings menu (F10)
- Smooth frame rates: the race logic runs at its fixed 60 Hz as before, and pictures between
  its frames show cars and camera in between (up to 240 frames a second); F12 shows the rate
- Light halos and lens flares hidden only where something really covers them
- The game's intro and race videos (Smacker) and the CD's music without a CD drive

Not there yet: widescreen, gamepads in the game, network play (the Mplayer version is not
installed). Linux builds are made by the release workflow but have not been played on yet.

## Building

```
cargo build --release
cargo run --release -- path/to/motor.exe
```

Without a path it sets up and uses `game_files` (next to motorhead.exe, else in the current
folder) as above. `cargo test --release` needs no game files. How it works and what is open:
`CLAUDE.md` and `PLAN.md`.

## License

MIT (the code here). The game and its files belong to their owners and are not part of this
project.

[iced-x86]: https://github.com/icedland/iced
[Cranelift]: https://cranelift.dev
[recomp_kit]: https://github.com/RadiatorTwo/recomp_kit
[releases]: https://github.com/RadiatorTwo/motorhead_rust/releases

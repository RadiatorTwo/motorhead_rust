# PLAN — Motorhead

## Now (2026-11-25, end of session)

- Last commit 388335a (main, 2 commits not pushed: 7e766ed Glide palettes, 388335a frames).
- Waiting for the user's new recording `rec3.txt` (+ `rec3.txt.1.state`) made with
  `$env:MOTORHEAD_RECORD="rec3.txt"; .	argetelease\motorhead.exe`. Then: replay it
  (`MOTORHEAD_INPUT=@rec3.txt`, with the user's settings, screenshots only) and go on with the
  graphics (pictures between logic frames, GUIDE 8b step 3; then speed, widescreen).
- Ask before every game start (each opens a window on the user's screen; runs are slow).
- Open: crash seen twice in 10 runs (0x46fc88 path object; diagnostic hook logs it); the
  menu part of a replay runs in real time (an early Enter may miss the intro skip); kit
  changes below not yet in recomp_kit (Glide palettes, input clock, savestate files).

The way through: recomp_kit's docs/GUIDE.md. Tick off what is done, note what was learned.
Framework in `crates/`: copied from recomp_kit 29f5eb9 2026-11-25.

## 1. Start

- [x] EXE's SHA-256 in `VERSIONS`, `Identity` filled in (title, exe, folder, registry key)
- [x] functions found: ~2200 by `find_functions` (no Ghidra export; Watcom, no debug names)
- [x] every instruction supported (`rtools missing`: only data after 0x4cde6f, hooked)
- [x] imports listed; all of motor.exe's and SMACKW32.DLL's implemented or stubbed

## 2. Window and menu

- [x] the game opens its window
- [x] intro videos (Smacker) play (`MOTORHEAD_SHOT`)
- [x] the main menu shows (2026-11-25)
- [ ] the menu answers to mouse and keys (`MOTORHEAD_INPUT`); Escape works
- [ ] first-start config: which renderer the game picks (want Glide), its graphics menu

## 3. Playing

- [x] the attract demo races in Glide (512x384) with textures (2026-11-25: P_8 textures and
  grTexDownloadTable were missing in glide.rs)
- [x] fixed logic rate (60 Hz, `src/frames.rs`: the clock 0x4bab5c a frame a call, the loop
  0x474a10 paced) and recordings that play the same: input in logic frames
  (`input_clock`), a savestate at the race's first frame next to the recording
  (`recN.txt.1.state`). Checked 2026-11-25: two replays the same for 3540 frames.
- [ ] a race of the player's own recorded with this (rec1/rec2 are from before: no savestate)
- [ ] pictures between logic frames (GUIDE 8b step 3; for now at most 60 fps)
- [ ] crash seen once in the demo, not again in ~6 runs: read at 0x2e4 in 0x46fc88 (a
  track/spline object with [+4] = 0; 0x4710xx frees such an object and zeroes [+4]).
  Watch for it.
- [ ] sound effects (DirectSound), CD music (cue: tracks start), video sound (waveOut: none)
- [ ] saving and loading (league, records)
- [ ] the whole game played through

## 4. Port

- [ ] setup from the CD image: the game is in an InstallShield cab (`Motor/disk1/data1.cab`):
  setup must take motor.exe and the Data group from it (by hand so far: `rtools iso
  --extract`, `rtools iscab --extract`, then Data\* and motor.exe into game_files/Motorhead,
  Movies into game_files/Movies)
- [ ] high resolution, widescreen, settings
- [ ] gamepad
- [ ] release (tag `v*`)

## Kit (taken over into recomp_kit 358ec43, 2026-11-25)

- `tools/new-game.ps1` does not copy `crates/indeo` (the workspace lists it): copied by hand.
- `image.rs`: Watcom's virtual sizes 0 (the raw size is the size) and .bss with a raw size
  but no raw data (was read from file offset 0). Without it discovery found 55 functions.
- `discover.rs`: `plausible` accepts cs:/es:/ds: prefixes (Watcom), wants a usual first
  instruction (a data word into the middle of an instruction blocked a real callback, 0x433764
  over 0x433624); data scanned at 2-byte steps in DGROUP (Watcom's 6-byte start-up tables),
  and DGROUP's code pointers made entries in `Program::build`.
- `lift.rs`: les/lds (offset only), aam; test in `tests/misc.rs` (`farptr`).
- `ddraw.rs`: surface QueryInterface adds a reference (COM; the game releases the first
  interface after the query and lost its flip chain); 8-bit display modes enumerated;
  GetDC of a surface is the surface (StretchDIBits draws into it), ReleaseDC shows the primary.
- `gdi32.rs`: palettes (CreatePalette, SelectPalette), StretchDIBits into surfaces, small stubs.
- `cdaudio.rs` + `misc.rs`: the mixer (CD volume) from rustschweine; winsock (no sockets),
  VERSION (none), DirectPlay 1 (unavailable), waveOut (no device).
- `kernel32.rs`: ReleaseMutex, CopyFileA/MoveFileA (game folder only), GetTempFileNameA,
  CreateProcessA/LoadModule (not started), console (none), GetProfileIntA.
- `user32.rs`: keyboard (GetKeyNameTextA, MapVirtualKeyA, ToAscii, Get/SetKeyboardState,
  layout name), clipboard (none), small ones. `advapi32.rs`: RegOpenKeyA.

## Kit (not yet taken over)

- `glide.rs`: palette textures (P_8, AP_88) through grTexDownloadTable(/Partial), converted
  again when drawn after the palette changed; grGlideGetVersion, grBufferNumPending.
- `input_clock.rs` (new): the time INPUT and RECORD count in (logic frames in a game's loop,
  input let through at frames' starts), `sync` points, `start` (savestate saved with a
  recording, loaded in its replay). `record.rs` (stamps, `sync@v`, `path`), `window.rs`
  (`Input::Sync`, `script_sync`, `script_file`), `user32.rs` (`poll_now`, gated input).
- `savestate.rs`: `save_to` / `load_from` a file; the heap fingerprint by the blocks' places
  only. `mod.rs` `MemoryCopy::read`: DLLs above the arena are guest memory (was refused).

## Learned (frames)

- The race's time step comes from QueryPerformanceCounter: replays drifted. Fixing the clock
  was not enough: the starting grid comes from `rand`, whose state the menus change, and the
  heap's order differs. A savestate at the race's first frame made replays exact.
- Dead ends: setting the clock to 0 at a race's start (0x474210 runs every frame of the
  countdown: it never ended; later at the first frame: the countdown waited for the clock to
  catch up with start times taken before); pacing at 0x47241c (it also runs many times a frame
  while loading); 0x4745b4 as the race's setup (not on the normal way to a race).

## Learned

- Watcom EXE with all virtual sizes 0: the kit's discovery found 55 functions until
  `image.rs` took the raw size (2026-11-25).
- The game's fonts and sprites are machine code in data files; the kit cannot run code it
  did not compile. Replacing the callers and interpreting the (tiny) loaded code was enough
  for the menu. A JIT for loaded code would be the general answer if more turns up.
- Smacker: compiling SMACKW32.DLL with the EXE works (`Game::dlls`, rebased); no Smacker
  decoder needed.

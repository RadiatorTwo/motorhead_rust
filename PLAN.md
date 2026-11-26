# PLAN — Motorhead

## Now (2026-11-26, end of session)

- Pictures between logic frames work: ~220–240 fps on the user's 480 Hz display (limit 240),
  cars and camera interpolated, the race log of rec3 unchanged. The user saw no problems
  (sound, gameplay). Next: the user plays a race and looks for stutter or lag (the names over
  the cars, the wheels, the HUD's map are not interpolated); then widescreen / resolution.
- Test runs need no asking any more (the user's standing go, 2026-11-25). Launch with
  `Start-Process -Wait` (GUI exe: PowerShell does not wait otherwise).
- Replays: `MOTORHEAD_INPUT=@rec3.txt` (+ `rec3.txt.1.state`), `MOTORHEAD_RACE_LOG=60`;
  compare the `race *` lines with an earlier run (1342 lines). Times after `sync@0` are ms.
  `RACE_LOG` itself causes ~50 ms spikes: measure speed without it.
- Profiling our own code: build with a link map into `target-map` (GUIDE 8a), run with
  `MOTORHEAD_SAMPLE=1 MOTORHEAD_SAMPLE_RAW=<file>` (a file per 2 s), resolve against the map.
- Open: rec3's savestate is no longer loaded ("heap blocks saved 460, now 504", the same up to
  block 12): seen 2026-11-26 before and after the kit merge, so older than it; the menu part
  of the replay going another way (real time), or a change to what the game allocates.
- Open: crash seen twice in 10 runs (0x46fc88 path object; diagnostic hook logs it); the
  menu part of a replay runs in real time (an early Enter may miss the intro skip); kit
  changes below not yet in recomp_kit.

The way through: recomp_kit's docs/GUIDE.md. Tick off what is done, note what was learned.
Framework in `crates/`: copied from recomp_kit 29f5eb9 2026-11-25, brought up to 800729a
2026-11-26 (three-way merge per file, no conflicts: settings menu, setup from cabinets, ...).

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
- [x] a race of the player's own recorded with this: rec3 replays the same twice over the
  whole race (2026-11-25)
- [x] pictures between logic frames (2026-11-26): the race's drawing 0x4775d8 runs again
  while the next frame is not due, the static data put back (`frames.rs`); cars and camera
  interpolated (`pictures.rs`)
- [ ] crash seen once in the demo, not again in ~6 runs: read at 0x2e4 in 0x46fc88 (a
  track/spline object with [+4] = 0; 0x4710xx frees such an object and zeroes [+4]).
  Watch for it.
- [ ] sound effects (DirectSound), CD music (cue: tracks start), video sound (waveOut: none)
- [ ] saving and loading (league, records)
- [ ] the whole game played through

## 4. Port

- [ ] setup from the CD image: the game is in an InstallShield cab (`Motor/disk1/data1.cab`):
  `cab_left_out: Some(&["Spawned"])` set (2026-11-26, the kit's setup installs from cabinets;
  groups: Program Executable Files, Data, Spawned = motorm.exe). Not tried yet: a setup run
  from the .bin/.cue into an empty folder; the movies stay on the CD (movies.pak)
- [ ] high resolution, widescreen, settings (the kit's F10 menu: page Display with fullscreen,
  resolution, anti-aliasing, textures, frame rate; not tried in the game yet)
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

## Kit (taken over into recomp_kit 800729a, 2026-11-26)

- `glide.rs` `read_depth`: grLfbLock of the depth buffer (AUXBUFFER, read) gives the z values
  (ooz) at the screen's size; was the picture's colours. Motorhead tests each light's
  visibility with it (0x4be458): halos and lens flares were seldom seen and flickered
  (2026-11-26, the user saw them all, steady).

- `glide.rs`: palette textures (P_8, AP_88) through grTexDownloadTable(/Partial), converted
  again when drawn with another palette than they were converted with (`palette_hash`: by
  content, not a count of changes; Motorhead switches palettes and had every texture
  converted and uploaded again at each switch, ~60% of a race); grGlideGetVersion,
  grBufferNumPending.
- `glide.rs` `read_screen` (grLfbLock for reading, grLfbReadRegion): the view brought down to
  the Glide screen's size on the GPU (blit, nearest) and only that read back (`read_fbo`):
  reading the whole window-sized target was ~30% of a race.

## Kit (not yet taken over)

- `tween.rs` `Statics`: a buffer per range (an image with a gap between its parts needs one
  `Statics` per part: Motorhead's DGROUP and .bss; reading the gap crashed).
- `sample.rs`: `SAMPLE_RAW` writes a file per 2 s (`<path>.<n>`), not only the last.
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
- The race's drawing was found through the renderer's function table: Glide's at 0x505eb0
  (found by searching the data for the address of the function that calls grBufferSwap),
  `[[0x77d95c] + 0x298]` points at it, +0x28 is the swap; its only caller in the race frame is
  0x4775d8, the frame's last call. Static call graphs miss such calls through tables.

## Learned

- Watcom EXE with all virtual sizes 0: the kit's discovery found 55 functions until
  `image.rs` took the raw size (2026-11-25).
- The game's fonts and sprites are machine code in data files; the kit cannot run code it
  did not compile. Replacing the callers and interpreting the (tiny) loaded code was enough
  for the menu. A JIT for loaded code would be the general answer if more turns up.
- Smacker: compiling SMACKW32.DLL with the EXE works (`Game::dlls`, rebased); no Smacker
  decoder needed.

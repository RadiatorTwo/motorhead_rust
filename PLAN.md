# PLAN — Motorhead

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

- [ ] the first race starts and plays (Glide; 3D drawers may jump into loaded code too)
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

## Learned

- Watcom EXE with all virtual sizes 0: the kit's discovery found 55 functions until
  `image.rs` took the raw size (2026-11-25).
- The game's fonts and sprites are machine code in data files; the kit cannot run code it
  did not compile. Replacing the callers and interpreting the (tiny) loaded code was enough
  for the menu. A JIT for loaded code would be the general answer if more turns up.
- Smacker: compiling SMACKW32.DLL with the EXE works (`Game::dlls`, rebased); no Smacker
  decoder needed.

# CLAUDE.md — Motorhead in Rust

Hand-over for future sessions. What is done and what is open: **PLAN.md** (keep it current).

## Goal

A native port of Motorhead (1998) with recomp_kit: the player's own EXE recompiled to native
code (Cranelift), Windows and DirectX in Rust (`win32` crate), playable on Windows and Linux.
Only code is shared; players supply their own game files. **Never commit anything derived
from the EXE** (translated code, extracted data). `game_files/` is git-ignored. Function
boundaries (`data/functions.json`) and our notes by address (`data/symbols.txt`) are fine.

## The user

- New to reverse engineering; explain in plain words, give a recommendation, not a survey.
- Speaks German; works in short sessions and watches usage limits.
- Confirm before anything outward-facing or destructive (pushing, deleting).
- Clean Rust first, performance later.
- The GPU is never the limit with this old game: a low frame rate is CPU side (game code,
  our code, draw calls, driver). Measure (recomp_kit GUIDE 8a).
- Better than the original: glitches are fixed whether or not the original has them.
- No environment variables for what the user tests by hand (cheat keys: Cargo feature
  `cheats`, never in a release; settings: menu or registry). Replay the user's recordings with
  exactly their settings.

## How it works

- The framework, copied from recomp_kit into `crates/` (`recomp`: discovery, lifter, JIT,
  runtime; `win32`: the Windows layer; `tools`: `rtools`). This project's own copy: change it
  freely, with a test for the lifter (`crates/recomp/tests/`). recomp_kit's docs/GUIDE.md is
  the way through a new port, docs/LESSONS.md the known traps. A fix that other games need
  too: also into recomp_kit (ask the user).
- `src/main.rs`: `Identity` (names, paths, registry key, CD) and `impl Game` (functions,
  self-modifying code, hooks, extensions).
- `src/hooks.rs`: game functions replaced by Rust (`call_original` runs the game's own).
- `data/symbols.txt`: what we know about the game's functions and data, by address.
- Switches (prefix `MOTORHEAD_`): `TRACE_API=1` every import call, `TRACE_MSG=1` window
  messages, `WATCH=1` where the game is every 2 s, `SHOT=file.png` screenshots,
  `INPUT="5000:click@320,240 8000:Enter"` scripted input, `RECORD=file.txt` records input,
  `NO_VIDEO=1`, `WINDOW=1280x960`, `COUNT=file` calls per function.

## Known facts about the game

- CD (`game_files/Motorhead.bin/.cue`, Mode1/2352 + 10 audio tracks, volume `Motorhead`):
  `Motor/disk1/data1.cab` (InstallShield 5: groups Data, Program Executable Files = motor.exe,
  Spawned = motorm.exe, the Mplayer version), `Movies/movies.pak` (Smacker videos, read from
  the CD: `game_files/Movies/movies.pak`), CD audio tracks 2-11 (music, `cdaudio.rs` reads
  the .cue). Install layout: motor.exe next to Cars, Data, Demos, Game, League, Sounds,
  Tracks, SMACKW32.DLL, a3dapi.dll. Settings in `game\MotorHead.CFG` (made at first start).
- `motor.exe` (1 081 344 bytes, SHA-256 `c27411fd...4d7a1c`): **Watcom C** (BEGTEXT 0x410000,
  DGROUP 0x4d0000, .bss 0x510000 without raw data; all virtual sizes 0), no debug names.
  Entry 0x4c5d0e, WinMain 0x471678. Functions found without Ghidra (~2200, `find_functions`).
- Renderers: DirectDraw for menus (640x480 or 640x400 at **8 bits**, palette), 3D through
  Glide (loads glide2x.dll), Direct3D (DX5 IDirect3D2) or PowerVR (sgl.dll); the choice is in
  the config. Sound: DirectSound (A3D/EAX optional: a3dapi.dll, CoCreateInstance fails).
- Videos: RAD's Smacker, `SMACKW32.DLL` compiled with the EXE (`Game::dlls`, at 0x5f400000:
  it wants 0x10000000, inside our heap). It draws each frame with GetDC on the primary
  surface + StretchDIBits (8-bit DIB, palette); its own sound uses waveOut (none yet).
- **Code in data files**: fonts (`Data/FONT*.FNT`) and sprites (`*.SPR`) are machine code
  (a run per glyph / per row). The game jumps into them; they are not compiled. Their
  callers are replaced (`font.rs`: 0x415878, `sprites.rs`: 0x410010, 0x41009a, 0x410113)
  and the loaded code runs in a small interpreter (`loaded.rs`: mov, add/sub/and/or/xor,
  inc/dec, lea, fst qword, ret/jmp). If other code jumps into loaded code: "call to unknown
  address" in the heap; find the caller and do the same.
- Glide (config `Renderer voodoo`, `TextureFormat 8`): textures are P_8 with a palette
  (grTexDownloadTable). In the menu the game plays a demo after a while (Demos\*.MDE): a
  race to look at without a recording.
- Race loop in WinMain (0x471678): 0x47241c (state machine, also loading) then 0x474a10 (a
  logic frame: clock 0x4bab5c, time step `[0x739f7c]` = float-ms difference x 0.001). Fixed
  at 60 Hz in `src/frames.rs`. Replays: `MOTORHEAD_INPUT=@recN.txt` loads
  `recN.txt.1.state` at the race's first frame (made by `MOTORHEAD_RECORD=recN.txt`). Test
  switches: `MOTORHEAD_RACE_LOG=n` (positions every n frames), `MOTORHEAD_STATE_DUMP=file`.
- Test runs of the game need no asking (the user's standing go): launch with PowerShell
  `Start-Process -Wait` (a GUI exe, PowerShell does not wait otherwise).
- Watcom runtime 0x4cde6f: `call +x` over a table, `pop edi; ret` (hooked: `pow10_table`).

## How it works (this game's own)

- `src/frames.rs` the race at 60 Hz logic, pictures between logic frames (0x4775d8 drawn
  again, static data put back) paced to the display (max 240 fps); `src/pictures.rs` cars
  and camera interpolated while drawing.
- Game functions in Rust, identical to the lifted original (each with a check switch that
  runs both and compares): `src/project.rs` 0x414e54 (`MOTORHEAD_CHECK_PROJECT=1`),
  `src/polygons.rs` 0x456d4c, its common ways, the game's function for the rest
  (`MOTORHEAD_CHECK_POLYGONS=n`: every n-th call on a memory copy; the window looks hung
  then). `src/fpu.rs`: x87 rounding as the lifter does it, for such code.
- `src/pad.rs` the pad in a race (analog steering, gas, brake), `src/controls.rs`
  the pad and mouse in the game's menus and its control options (key array 0x51c3d4).
- `src/graphics.rs` the game's graphic options: only Glide offered, the port's resolutions in
  its Screen size list (kept the same as F10's), Glide always opened at 1024x768.
- `src/loaded.rs` the interpreter for loaded code; `src/font.rs`, `src/sprites.rs` the
  replaced drawers (each the game's loop, registers carried as in the original).
- Framework changes made here (not yet in recomp_kit, see PLAN.md "Kit"): Watcom sections
  (`image.rs`), discovery (`discover.rs`), les/lds/aam (`lift.rs`), win32 imports.

## Next steps

See PLAN.md.

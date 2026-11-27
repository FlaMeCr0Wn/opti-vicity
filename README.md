# VIctory Unificado (Yaekai 3070 + performance overlay)

A fork of [Yaekai/victory-scarlett-3070](https://github.com/Yaekai/victory-scarlett-3070) (master as of
2026-11-26), which is [Op-lite/victory-scarlett](https://github.com/Op-lite/victory-scarlett) plus 8 GB NVIDIA work. On
top of it, this fork adds:

- **a performance overlay**, ported from Dolberth's Neptune;
- **a launcher tuned for 12 GB+ NVIDIA cards** that also fixes the black or half-loaded first runs.

The goal was to combine the best parts of the forks tested on one PC: Yaekai/scarlett's texture loading and picture,
and Neptune's overlay. Their READMEs follow below, unchanged.

**This repository contains no game files, code, or assets.** You need your own dump of the title, meaning the folder
with `game_scarlett_master.exe`. This fork is not affiliated with or endorsed by the title's developer or publisher. It
is licensed GPL-3.0-or-later, like everything it is built on.

> **PT-BR (resumo):**
> - **O que é:** o fork do Yaekai (scarlett + ajustes NVIDIA) com o overlay de desempenho do Neptune. O F10
>   mostra/esconde o overlay e mostra FPS, ms, draws, CPU, GPU, RAM e VRAM.
> - **O launcher:** `Play-Unificado.cmd` roda em Series S com texturas completas (para placas de 12 GB+) e corrige a
>   primeira execução preta ou carregando aos poucos.
> - **Como usar:** compile com `cargo build --release --locked -p victory`. Na primeira vez, arraste a pasta do seu dump
>   para `Play-Unificado.cmd`; depois disso, basta dar dois cliques.
> - **Primeira execução:** a primeira sessão é mais lenta enquanto o cache se forma. As seguintes carregam direto.
> - **Placas de 8 GB:** use `-MipDrop 1`.

## What's added on top of `Yaekai/victory-scarlett-3070`

### Performance overlay (`crates/victory-gpu/src/overlay.rs`, `pixels.rs`)

The panel sits in the top-left corner of the game window. It shows:

- FPS and ms for the frame, plus the 30 s high/low/average;
- draws per frame;
- CPU % and GPU % (GPU is system-wide, from Windows performance counters);
- RAM used/total;
- VRAM used/budget;
- the CPU and GPU names, and the build's commit.

How it is drawn:

- It is a separate topmost, click-through, layered Win32 window, drawn with GDI on its own thread and refreshed every
  500 ms. So it costs the frame nothing and never shows up in the game's own image.
- It follows the game window and hides when the game is minimized or another window is in front.

Controls:

- **F10** shows/hides it while the game is in front.
- **`VICTORY_OVERLAY=0`** (or `-NoOverlay`) turns it off.

Where it comes from:

- It is ported from [Dolberthi/neptune-dolberth-nvidia-8gb](https://github.com/Dolberthi/neptune-dolberth-nvidia-8gb).
  That repo credits the overlay to zm0kie.
- Its F12 screenshot code was left out.
- The hooks are two lines in `crates/victory-gpu/src/host.rs`: frame stats from `Meter::frame`, and the start in
  `presenter()`.
- `crates/victory-gpu/build.rs` stamps the commit into the build (`VICTORY_BUILD`; "unknown" when built outside a git checkout).

### `Play-Unificado.cmd` / `scripts/play-unificado.ps1`

It uses Yaekai's `scripts/play-3070.ps1` settings (Series S), with these changes:

| Setting | play-3070 | play-unificado | Why |
|---|---|---|---|
| `VICTORY_TEXTURE_MIP_DROP` | 1 | **0** (`-MipDrop 1` for 8 GB) | Full textures. The top level only needs to be dropped on 8 GB cards. |
| `VICTORY_PIPELINE_WAIT_MS` | 20 | **2000** | With 20 ms, a draw whose pipeline is still compiling is skipped. On a first visit that meant black screens, broken text, and people without heads. |
| `VICTORY_CACHE_DIR` | default | **`%LOCALAPPDATA%\victory\cache-unificado`** | One fixed cache folder for this build. Saves stay in the default place. |
| Prepare after a new build | `-Prewarm` by hand | **automatic** | Runs `victory shaders <shaders.rpf>`, then `victory warm --used`. The second step compiles ahead every pipeline the game was seen using, as written down by `VICTORY_PIPELINE_BOOK=1`. |
| `VICTORY_LOG` | info | warn | Less console output. |

How the first runs go:

- **First session:** slower while it fills the cache, and it writes down the pipelines it meets.
- **Next start:** the launcher compiles all of those pipelines ahead of time. On the test PC that was 2,047 pipelines
  in 134 s, about 2.5 minutes with the shader pass.
- **From then on:** the game loads complete from the first frame.

Options:

- `-SeriesX` uses the bigger console: sharper and with a larger texture budget, but slower.
- `-Prepare` only translates the shaders and compiles the pipelines ahead, then quits.
- `-More @{ VICTORY_X = 'y' }` sets anything else.

## Build and run

1. Install the tools once: [Rust](https://rustup.rs), plus the Visual Studio 2022 C++ build tools with a Windows SDK.
2. Build:

   ```powershell
   cargo build --release --locked -p victory
   ```

   Cargo prints warnings and PowerShell may report a non-zero exit because of them. Check for `Finished` and
   `target\release\victory.exe`.
3. Drag your dump's folder onto **`Play-Unificado.cmd`**. After that, a double-click is enough: the folder is remembered
   in `game-folder.txt`.

A prebuilt `victory.exe` placed in `bin\` (or next to this README) is also picked up.

## Measurements (one PC, for reference)

**Test PC:**
- Ryzen 7 5800X3D, RTX 3080 Ti 12 GB, 32 GB RAM, Windows 11.
- **Resizable BAR off**, so the `VICTORY_WINDOWS_BAR_MB` / GPU-upload paths were not usable.

**Method:**
- Every run used the same save, restored fresh each time, in the prison area.
- Each measured run lasted 5 minutes, after a warm-up run.
- Input was scripted to get past the menus.
- Results are the median ms per frame. Because the scene drifts between runs, they are also compared by draw count.

| Build / settings | Median ms | Draws (median) | Notes |
|---|---:|---:|---|
| **This fork, play-3070 set, Series S, MipDrop 0** | **54.6** | 3,846 | CPU preparation 36 ms/frame. The picture was correct. |
| cool_fox (prebuilt) + `LEVELS_ON_DEMAND=0`, `CARD_BUFFERS_MB=1024`, `CARD_WINDOWS_MB=256` | 58.6 | 5,415 | CPU preparation 46 ms/frame. |
| cool_fox defaults (pareto-residency profile) | ~505 | n/a | GPU-bound, about 2 fps. |
| Op-lite/victory-scarlett, its own launcher (Series X) | 288.7 | n/a | |
| gleaf-6/VIctory-7900XTX- with the cool_fox settings above | 76.6 | 4,499 | |
| Dolberthi/neptune-dolberth-nvidia-8gb, run-neptune.ps1 | 84 | 4,491 | Different spot in the scene (the auto-input differed). |
| rorodev6/victory-nvidia-megafork, play-nvidia.ps1 (540p) | 81–92 | 3,446 | |

By draw count, this fork against the tuned cool_fox (ms per frame):

| Draws per frame | This fork | cool_fox tuned |
|---|---:|---:|
| 2,500–3,500 | 47.4 | 55.0 |
| 3,500–4,500 | 51.8 | 72.3 |
| 4,500–5,500 | 62.8 | 57.1 |
| 5,500+ | 89.0 (few samples) | 60.5 |

**What the numbers show on this PC:**
- **The frame is CPU-bound.** Each frame copies about 115 MB of unified memory up and about 60 MB back. Lowering the
  resolution (megafork's 540p) gained nothing.
- **Tried and not adopted:**
  - **Neptune's "smart copy-back"** (skipping copy-back for large shadows) lowered the copies a lot, but it drops real GPU
    writes: 1,333 of 1,346 copied-back pieces had changed bytes.
  - **`VICTORY_LAND_THREADS=8`** gained about 1 ms but ran out of texture room ("card has no room for more"), which made
    the picture dark and ghosted.

## History of this fork

- `ee36438` imports Yaekai/victory-scarlett-3070 master as downloaded on 2026-11-26. The original repo's git history is
  not included, so the README sections below say what it changed.
- The next commit adds Neptune's performance overlay.
- The commit after that adds the launcher and this README.

---

# victory-scarlett-3070 (8 GB NVIDIA / RTX 3070 Fork)

A fork of [Op-lite/victory-scarlett](https://github.com/Op-lite/victory-scarlett) (itself based on
[fxceless/victory-fork](https://github.com/fxceless/victory-fork) and [Marvisak/VIctory](https://github.com/Marvisak/VIctory))
tuned for **8 GB NVIDIA GPUs** (tested on an RTX 3070 8 GB, 32 GB RAM, Windows 11). The upstream
`victory-scarlett` README follows below.

**This repository contains no game files, code, or assets.** You need your own copy of the title.
It is not affiliated with or endorsed by the title's developer or publisher. Licensed GPL-3.0-or-later.

## What's Added on Top of `Op-lite/victory-scarlett` (`835ca60`)

- **8 GB VRAM & NVIDIA stability:**
  - `VICTORY_TEXTURE_MIP_DROP=1` (`resource.rs`) — drops top mip levels of large placed textures (~75% less texture VRAM).
  - Immediate idle VRAM eviction (`fxceless#1269`, `draw.rs`).
  - NVIDIA 1M descriptor-heap cap fix (`VICTORY_DYNAMIC_SLOTS=524288`, `descriptor/host.rs`).
  - Prelude allocator cap & recycling (`VICTORY_MAX_PRELUDES=32`, `host.rs`).
  - Shadow scratch buffer reuse (`fxceless#1350`, `shadow.rs`).
  - `TITLE_PAGES` working-set reporting (`xmem.rs`).
- **Visual fixes:**
  - `R9G9B9E5` -> `R11G11B10_FLOAT` format stand-in (`resource.rs`, fixes purple/violet lighting & vegetation).
  - Ambient leftover state fix (`ambient.rs`).
  - Integer render-target unblending (`fxceless#501`, `pipeline.rs`, `pipeline/host.rs`).
  - `VICTORY_SKIP_PIPELINES='02384a60fda6f2d0,731a39171b568298'` in `scripts/play-3070.ps1` (skips broken water/heat-haze blocks and decal-heap defrag).
- **Performance & pacing:**
  - 60 FPS frame-interval unlock (`VICTORY_FRAME_INTERVAL_MS=16.667`, `frame_rate.rs`, `frame.rs`, `run.rs`).
  - Async stderr logging (`victory-core/src/logging.rs`).
  - Dedicated RTX 3070 8 GB launcher (`scripts/play-3070.ps1`, preserves default `%LOCALAPPDATA%\victory` saves & cache).

## Build and Run

```powershell
cargo build --release --locked -p victory

# First run (if you don't have a shader cache yet):
.\scripts\play-3070.ps1 -Game 'C:\path\to\game_scarlett_master.exe' -Prewarm

# Normal runs:
.\scripts\play-3070.ps1 -Game 'C:\path\to\game_scarlett_master.exe'
```

---

# Scarlett fork (branch `scarlett-fox`), state of 2026-11-26

## How to start the game (the short version)

You need your own dump of the game: a folder that has `game_scarlett_master.exe` in it. **This repository contains
no game files, code or assets.**

1. **Install the tools once:** [Rust](https://rustup.rs) and the Visual Studio C++ build tools with a Windows SDK.
2. **Build once.** Open PowerShell in this folder and run:

   ```powershell
   cargo build --profile diag -p victory --features diagnostics --locked
   ```

3. **Start the game.** Drag your game folder onto **`Play-Scarlett.cmd`**.
   After the first time, just double-click `Play-Scarlett.cmd`: it remembers the folder.

That is all. `Play-Scarlett.cmd` sets every setting this branch runs with, in Series X mode. The first start
takes a few extra minutes while it prepares the game's shaders (see below); after that it starts straight away.

Good to know:

- **Shaders are prepared ahead, by itself.** Before the game starts, the launcher translates all of the game's
  shaders once (`victory.exe shaders <your game folder>\shaders.rpf`). You will see a console window counting
  for a few minutes. It happens only when it is needed:
  - on the first start,
  - after you build a new `victory.exe`,
  - after you change a setting that changes the shaders (`-RayTracing`, `-NoProbeRays`).

  It is remembered in the cache folder (`scarlett-shaders-ahead.txt`), so every other start skips it. Without
  this step the game would translate shaders while you play: slow, with objects missing and faces drawn wrong
  for that session. To run only this step: `.\scripts\play-scarlett.ps1 -OnlyPrecompile`. To skip it:
  `-NoPrecompile`. To force it again, delete `scarlett-shaders-ahead.txt` from the cache folder
  (`%LOCALAPPDATA%\victory-scarlett\cache` unless you chose another).
- **The first session still stutters a little** while the graphics driver builds its own pipelines; later
  sessions are smoother.
- **Controls:** an Xbox controller works. On the keyboard: W A S D move, I J K L camera, Enter = A, Backspace = B,
  X and Y, Z / C = left / right trigger, Q / E = LB / RB, arrows = D-pad, Esc = Menu, Tab = View.
- **Do not use `Play-VIctory.cmd`** for this: that is the base's launcher, with Series S and other settings.
- **Ray tracing** is off by default (the game's own, much slower). To turn it on, start it from PowerShell:

  ```powershell
  .\scripts\play-scarlett.ps1 -RayTracing
  ```

- **The violet glow on bushes, trees and palms** that the base shows with ray tracing off is gone by default
  (`-NoProbeRays` brings the base's picture back). The first start after this update re-translates shaders.
- **More speed:** `.\scripts\play-scarlett.ps1 -Hooks` turns the memory hooks on (about 10-15 ms a frame less in
  the opening scenes). They are off by default: they can show a few small textures with old contents for a moment.
- More options are listed at the top of `scripts\play-scarlett.ps1` (other cache or save folders, a release build,
  automatic start-screen presses, any single setting through `-More`).

## What this is

This branch is codec-d/cool_fox at `f4b48ad` (itself avatarmanz/VIctory-7900XTX `0a43566` plus six commits) with the
work of a local fork ("Scarlett") ported onto it. Target PC: Ryzen 7 9800X3D, 32 GB RAM, RTX 4080 SUPER 16 GB,
2560x1440, Series X mode. Goal: a stable 30 frames a second with no loss of picture. It is not there yet.

## Measured (6-minute opening route, diagnostics build, ray tracing off, with `-Hooks`)

| | cool_fox as it was (floor) | this branch, current settings |
|---|---|---|
| ms per frame, 1,000-2,500 draws | 50-55 | 35 |
| ms per frame, 2,500-4,000 draws | 50-53 | 44 |
| ms per frame, over 4,000 draws | 59-66 | 56 |
| frames over 1.5x the usual | 7-8 % | 2.3 % |
| 5-second periods with a frame over 500 ms | 18-21 | 10 |
| frames drawn in the 6 minutes | 4,400-5,900 | 8,385 |
| texture copies made again in the first 4 minutes | about 6.4 GB | 14 MB |
| memory commit, peak | 37 GB | 33 GB |

Heavier hand-played scenes (a prison yard at 5,500-8,600 draws, the open world) run at about 48-62 ms a frame
(16-21 fps). Identical runs on the test PC differ by 13-18 ms a frame at the same draw count, so single-run frame
times prove little; the memory, re-upload and freeze figures and each switch's own counter line are what is solid.

Where the time goes now (ray tracing off): the graphics card is mostly idle. One thread of the layer, the one that
hands each frame's work to the card, is busy for the whole frame (32-44 ms: buffer refreshes 12-14, copies in the
card's memory 7-8, other list steps about 6, write-watch asks about 4).

## The settings `Play-Scarlett.cmd` uses

All settings are environment variables read at start; `scripts\play-scarlett.ps1` holds the list, in groups:

- cool_fox's launcher set, **without** Series S, `VICTORY_THIN` and the stand-in ambient light, and with the
  keep-the-card-busy loop off (`VICTORY_CARD_AWAKE=0`: it held the card at 97-99 % in menus for no gain).
- `VICTORY_LEVELS_ON_DEMAND=0`: textures keep only the levels their views show. cool_fox's launcher set it to 1,
  which made every texture keep its whole chain on the card. Off: re-uploads near zero, about 4 GB less commit,
  fewer freezes. This was the largest single gain.
- The memory hooks, only with `-Hooks` (see above).
- `VICTORY_POOL_DISCARD=1`, `VICTORY_FILL_TITLE=1`, `VICTORY_PARTIAL_DELTA=1`, `VICTORY_BAKE_DELTA=1`,
  `VICTORY_MAP_LOCK_FINE=1`, `VICTORY_POOL_CAP=1`: unused pool pages given back under low RAM, no read waits for
  fills, unchanged texture levels not uploaded again, the mapping lock taken per range, the texture pool capped.
- `VICTORY_TABLE_MEMO=1`: a descriptor-table entry made ready earlier in a command list is not made ready again
  (81 % of about 54,000 entries a frame; verified against the full path). `VICTORY_LIST_ALIAS_EXACT=1`: a texture
  read through a view in another format follows its source's writes within a list (the base read a stale copy once
  a frame in the title's lighting filter). `VICTORY_LIST_HOLD=1`: a view copied into a new table is held once
  per list, not once per slot.
- Picture waits: `VICTORY_BAKE_TEXTURES_MS=10000` (the title's character bake waits up to 10 s for its textures;
  1.5 s by default: faces and skin came out wrong in some runs) and `VICTORY_PIPELINE_WAIT_MS=500` (a draw waits
  for a pipeline still being made instead of being dropped after 20 ms).

## What was added to cool_fox

Always on:
- Unattended runs: `VICTORY_AUTO_INPUT` (a script of pad presses keyed on draws per frame, to pass the start
  screens), a window caption with fps, ms and draws, `VICTORY_FRAME_LOG` (per-frame breakdown by thread, long frames
  listed with their cause), `VICTORY_HANG_WATCH_S`, `VICTORY_PRESENT_SKIP` for runs with the display off.
- Picture: 16-bit float values moved through unsigned-integer views (`VICTORY_HALF_VIEWS`, `VICTORY_D16_BITS`; fixes
  frozen facial animation and a blurred disc in cutscenes), a NaN guard on colour exports (`VICTORY_NAN_GUARD`),
  early depth, pipelines made for the render-target formats actually bound (`VICTORY_TARGET_VARIANTS`), an alias copy
  after an upload earlier in the same list, views keep the title's minimum LOD. 26 compute programs the base refused
  now translate (hair shadow masks, water volumetrics, particle emits, decals).
- Meters (diagnostics builds): uploads by cause, read waits by reason, mapping-lock readers, memory housekeeping,
  texture detail, the game's chunk pool, page kinds by the title's own table, per-pass GPU times
  (`VICTORY_PASS_TIMES=1`, `=2` serial), list calls and draw set-up by part (`VICTORY_LIST_CALLS=1`).

Switches, off by default (the base's behaviour is kept unless a run showed a gain); the ones in the launcher are
described above. Others:
- `VICTORY_PAGE_POOL=1`: textures on pages of reserved resources (about 1.2 GB less texture memory, no frame cost).
  Its page mover `VICTORY_PAGE_MIGRATE` loses the device on the test card and must stay off (`tools/docs/fox-port-V.md`).
- `VICTORY_STAGING_REST_MS`, `VICTORY_STAGING_PACK`, `VICTORY_SEND_SPARE_MB`, `VICTORY_WINDOW_KB`,
  `VICTORY_WINDOW_TAILS`, `VICTORY_HOLDERS`: memory trims; about 1-2 GB less commit, not in the launcher.
- `VICTORY_CLEAN_STAMP`, `VICTORY_ASK_KEPT_EARLY`, `VICTORY_FILL_FROM_PLACED`, `VICTORY_SIGNATURE_LOCAL`,
  `VICTORY_BIND_SAME`, `VICTORY_LIST_BATCH`, `VICTORY_FAULT_NOTE_ONCE`, `VICTORY_BASE_REUSE`, `VICTORY_MAP_DEFER`,
  `VICTORY_REMAP_SAME`, `VICTORY_CHUNK_FINAL`, `VICTORY_BIND_FILTER`, `VICTORY_HOIST_LOADS` and more: built, exact,
  no gain shown (or a loss: `VICTORY_LIST_BATCH`), or untested. `tools/docs` says which.
- Tried and rejected on this base: `VICTORY_THIN=1` (no gain here, twice the long frames), `VICTORY_WINDOWS_BAR_MB`
  (game memory in VRAM through Resizable BAR: no gain while the card is idle, 1.4 GB of VRAM), a release build
  without diagnostics (no faster), `VICTORY_DISPLAY_MODE=2560x1440` (the title still builds its 4K targets),
  `VICTORY_SHADER_LOOP_LIMIT=1024` (black faces), `VICTORY_BVH_COUNT32` (breaks ray tracing), `VICTORY_UNMAP_EXACT`,
  `VICTORY_TEXTURE_RAM_CAPS=0`, `VICTORY_VRAM_RAW_PCT`.

## Known problems

- Fixed by default: the violet glow on bushes, trees and palms and the bright blue awnings with ray tracing off.
  The base leaves out the title's probe-activation program, and the light probes then keep whatever their
  memory held. `VICTORY_PROBE_RAYS=1` (set by the launcher) runs it and answers every ray test as for an empty
  structure, so each ray ends with the title's own miss result; no cost measured.
- Purple patches can linger for a few seconds in some scenes; magenta or blue streaks, shiny skin or a dark band
  across the eyes appear on characters in some runs. Candidates and evidence: `tools/docs/shading-gaps-audit.md`
  (a material-class depth plane was read before it was current, once a frame: fixed by `VICTORY_PLANE_NOW=1`,
  set by the launcher; two compute passes make NaNs: ambient occlusion and a far-depth downsample, being traced).
- Freezes of about a second at scene loads: the title's page-mapping copies cost one map and one unmap system
  call per 64 KiB page, and those calls get 50 times slower when free RAM falls under about 3 GB (the title
  and the layer commit 31-34 GB). `VICTORY_POOL_DISCARD=1` (in the launcher) prevents most of them: without
  it a 6-minute run had 39 freezes instead of 7. Close other programs before playing on a 32 GB PC.
  `tools/docs/freeze-analysis.md`.
- The memory hooks (`-Hooks`) can draw a few small textures with old contents until their own check catches it
  (7 in a 6-minute run). Off by default until fixed.
- Heavy scenes sit at 16-21 fps, and driving in the open world drops lower (50-90 ms a frame at 9-12 thousand
  draws): there the title's five recording threads are the limit, each bind costing about 22 us in the layer.
  On lighter scenes the layer's submitting thread is the limit (`VICTORY_REFRESH_STALE=1` is verified exact but
  gains no time yet).
- Meshes can stretch for a second or two in busy scenes (cause not found).
- Ray tracing (`-RayTracing`) is off by default: sharper and better lit, but the card then needs about 84 ms a
  frame (`tools/docs/rt-parked-T.md`).
- Texture pop-in remains, less than before; the card reaches 14-15 GB of 16 in long sessions.
- Started without the launcher (or with `-NoPrecompile`) on an empty shader cache, a first run can show missing
  or black faces; the launcher's prepare-ahead step avoids that.
- In Series X mode the title always renders at 2560x1440 and upscales to 3840x2160 itself.
- Two pipeline creation methods are not implemented (the title falls back to ordinary containers).
- Fixed since the last state: a crash (a use after free in a meter thread that runs with the hooks).

## Where things are

- `Play-Scarlett.cmd`, `scripts/play-scarlett.ps1`: the launcher.
- `tools/docs/picture-settings-checklist.md`: every picture setting against another cool_fox build's.
- `tools/`: the measuring scripts (`run-auto-coolfox.sh`: hands-off run with watchdog; `run-compare.sh`,
  `run-freezes.sh`, `run-focus.sh`), notes (`tools/docs`: what was measured, tried and rejected; `fox-port-*.md` list
  every ported switch; `cool-fox-willy-review.md` and `left-out-audit.md`: a settings audit and what the layer
  leaves out or approximates), `tools/baseline-coolfox/README.md` (the floor and how to compare against it).
- The older Scarlett tree (soldsawn/victory-nvidia-8gb base) is on branch `scarlett`.

The base's own README follows.

---

# VIctory on an RX 7900 XTX

## Run the codec_d Windows build

Download and extract the Windows x64 ZIP from the [private releases](https://github.com/codec-d/cool_fox/releases/latest). Open PowerShell in the extracted folder and point the launcher at your own complete Xbox dump.

```powershell
$env:VICTORY_GAME_DIR = 'D:\Games\MyDump'
.\Play-VIctory.cmd pareto-residency
```

The launcher selects Series S and the RTX-tested residency settings. It builds its shader cache locally under `%LOCALAPPDATA%\VIctory\cache-pareto`. Set `VICTORY_CACHE_DIR` to reuse an existing cache outside the checkout. Windows x64, an AVX2 CPU, and a Direct3D 12 GPU with shader model 6.6 and mesh shader support are required.

To build from this checkout, install the pinned Rust toolchain and the MSVC C++ build tools with a Windows SDK, then run `cargo build --release --locked -p victory`. The same launcher finds `target\release\victory.exe`. The sections below describe the upstream AMD fork and its separate launcher.

A fork of [VIctory](https://github.com/Marvisak/VIctory), a compatibility layer
that runs an Xbox Series title natively on Windows from files the user supplies.
This one is about **speed on AMD RDNA 3**: the world at 30 frames a second on
one PC, without giving up the picture. It is not there yet.

**This repository contains no game files, code or assets.** You need your own
copy of the game. It is not affiliated with or endorsed by the game's developer
or publisher.

## Where it stands (2026-11-25)

Tested on one machine only: Windows 11, Intel i9-12900K, 64 GB RAM, Radeon
RX 7900 XTX, 1440p, the Series X profile.

| | |
|---|---|
| Menus, videos, loading screens | 30 fps |
| Prologue, indoors | 50-75 ms a frame (13-20 fps) |
| First mission with the boat | 46-53 ms (19-22 fps) |
| Street chase with many cars and people | 100-165 ms (6-10 fps), single frames far longer |
| Outdoors on foot, the camp | 60-90 ms |
| Flying | 42-57 ms |
| Longest session so far | 41 minutes, no crash |
| Memory | 40-46 GB of private memory: **64 GB of RAM recommended** |

It started at 280 ms a frame in the same scenes two days earlier.

What is known to be wrong:

- Trees, plants and far buildings flicker outdoors in some places.
- Rooms with little direct light are too dark without `-RayTracing`.
- Textures of what just came into view appear late while the title streams.
- The first run with a new driver or a new build stutters while the driver makes
  pipelines (`victory warm` makes them ahead).
- Recording a frame in the world makes the title's own watchdog end it.

## How to run it

You need Windows 10/11 x64, a CPU with AVX2, a Direct3D 12 GPU with shader
model 6.6 and mesh shaders, Rust (the toolchain is pinned), the MSVC build
tools, and your own dump of the title.

```powershell
git clone https://github.com/avatarmanz/VIctory-7900XTX
cd VIctory-7900XTX
copy victory.example.toml victory.toml      # then set your paths, outside this folder
cargo build --locked --profile diag -p victory --features diagnostics
scripts\play.ps1 -Exe <path of your copy's executable>
```

`scripts\play.ps1 -Safe` leaves out the newest, faster ways; try it first when
something is wrong. `-RayTracing` turns the title's ray-traced indirect light on.

Before the first run, ahead of time (both can be stopped and go on later):

```powershell
target\diag\victory.exe shaders <the shader archives>   # every shader translated, minutes
target\diag\victory.exe warm --used                     # pipelines earlier runs drew with, for a new driver or build
```

Commands, debugging switches and upstream's own description:
[docs/README-fxceless.md](docs/README-fxceless.md). Every `VICTORY_...` switch is
described in a comment where it is read.

## What is different here

In short; the reasons and the measurements are in [docs/findings.md](docs/findings.md).

- **List calls are written down, not carried out, on the title's threads**
  (`VICTORY_THIN`): eight threads of ours translate them.
- **Submissions are prepared by a thread of ours and joined**
  (`VICTORY_ASYNC_SUBMIT`, `VICTORY_JOIN_PARTS`), with what can be spread over
  threads spread: refreshes, compares (AVX2), descriptor records, landings.
- **What shaders wrote comes back beside the write guards**, by the dwords that
  changed, on a thread of its own (`VICTORY_RELAY`, `VICTORY_LAND`).
- **Buffer fills that need bytes still on the card are done on the card**
  (`VICTORY_CARD_STEPS`) instead of waiting for it.
- **The title's writes are caught more cheaply**: one view opened at a fault,
  blocks that rest, places in the title's code that look before they write.
- **Textures go up on a copy queue of their own** and a slot shows one only after
  its upload ran, which also ended a device loss that came every 15 minutes or so.
- **Boats float**: textures that shaders write and the title's CPU reads go back
  to title memory (from fxceless and Geckomacabre, on by default here).
- **Frame record and replay** with per-pass times and shader variants compared
  turn by turn ([docs/replay.md](docs/replay.md)).

## Credits

This is a tree of other people's work with more on top. The history is kept, and
commits taken from elsewhere name the commit and its author.

- **[VIctory](https://github.com/Marvisak/VIctory)** by Marvisak and its
  contributors: the loader, the runtime services, the Direct3D 12 back end and
  the shader recompiler.
- **[fxceless/victory-fork](https://github.com/fxceless/victory-fork)**: the fork
  this one started from, and the people whose pull requests it carries.
- **[DexNom/VIctory](https://github.com/DexNom/VIctory)**: ray tracing fixes,
  cube gradients, sparse textures.
- **[Belobug/VIctory-Belobug-AMD](https://github.com/Belobug/VIctory-Belobug-AMD)**:
  the ambient light idea, pipeline workers, and work by romainhedouin.
- **[FQY55kai/ZJOU-VICTORY](https://github.com/FQY55kai/ZJOU-VICTORY)**.
- **[myokex/victory-amd](https://github.com/myokex/victory-amd)**: the guard over
  every view of a page (with Spexivus, joroyt and tightfall), and Joshua Odom's
  reads into title memory.
- **Geckomacabre** and **cxy0072019-sketch**: the texture read-back.
- **[XCL](https://github.com/ivall/xcl)** (GPL-2.0-only) was read for how the same
  title behaves under another layer; no code of it is here.
- This fork's changes were made with [Claude Code](https://claude.com/claude-code).

## License

GPL-3.0-or-later, as VIctory. See [LICENSE](LICENSE); the one file that is not
covered is named in [docs/asset-policy.md](docs/asset-policy.md).

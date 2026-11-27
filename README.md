# opti-vicity

> ### ▶ To play: [download the latest release](https://github.com/FlaMeCr0Wn/opti-vicity/releases/latest), extract it and open `opti-vicity.exe`
>
> **Para jogar:** [baixe a release mais recente](https://github.com/FlaMeCr0Wn/opti-vicity/releases/latest), extraia e
> abra o `opti-vicity.exe`. É o único arquivo que você precisa abrir (deixe a pasta `bin` ao lado dele).
>
> The code on this page is the source. You do not need it to play. It is only for building opti-vicity yourself
> (see [Build from source](#build-from-source)).

Runs your own Xbox Series dump of the title on a Windows PC, with:

- a **one-window launcher** (drag the game folder in, press Play);
- a **performance overlay** (F10) that also shows the console, the render and output sizes, the upscaler and frame generation;
- a **lower render resolution** through AMD FSR 3.1 (any card) or NVIDIA DLSS (RTX), and **frame generation**: AMD FSR 3 (2x, any card) or DLSS (2x to 6x, RTX 40/50);
- an **options menu in the game** (F1) that switches frame generation at once;
- **mouse and keyboard** controls;
- **first runs that load completely**.

opti-vicity is built on the VIctory line of projects. See [Credits](#credits).

**This repository contains no game files, code, or assets.** You need your own dump of the title: the folder with
`game_scarlett_master.exe` and `shaders.rpf`. opti-vicity is not affiliated with or endorsed by the title's developer or
publisher. It is licensed GPL-3.0-or-later, like everything it is built on.

> **PT-BR (resumo):**
> - **Como jogar:** baixe o zip da Release, clique direito > Propriedades > Desbloquear, extraia e abra `opti-vicity.exe`.
>   Arraste a pasta do jogo para a janela e clique em **Jogar**.
> - **Primeira vez:** o launcher prepara os shaders (com barra de progresso) e a primeira sessão é mais lenta. Feche e
>   abra de novo, e daí em diante carrega completo.
> - **Overlay:** o F10 mostra/esconde FPS, ms, draws, CPU, GPU, RAM e VRAM, e também o console, a resolução (interna → saída), o
>   upscaler e a geração de quadros.
> - **Upscaler e geração de quadros (FG):** no launcher escolha o upscaler (do jogo, AMD FSR 3.1 ou NVIDIA DLSS), a resolução
>   interna e o FG (AMD FSR 3 2x em qualquer placa; DLSS 2x a 6x nas RTX 40/50). No jogo, **F1** abre as opções: o FG liga e
>   desliga na hora; o resto vale ao reiniciar ("Salvar e reiniciar agora" reabre o jogo sozinho). O DLSS precisa dos arquivos
>   da NVIDIA: botão "Baixar arquivos do DLSS" no launcher (baixa ~263 MB do GitHub da NVIDIA uma vez).
> - **Mouse:** clique na imagem (ou F9) para olhar com o mouse; F9 ou Alt+Tab solta. WASD anda, botão esquerdo/direito =
>   atirar/mirar, Espaço ou Enter = A.
> - **Placa de 8 GB:** escolha "8 GB" no launcher.

## Quick start

1. Download `opti-vicity-<build>-windows.zip` from [Releases](https://github.com/FlaMeCr0Wn/opti-vicity/releases).
2. Right-click the zip > Properties > tick **Unblock** > OK, then extract it anywhere.
3. Open **`opti-vicity.exe`**. SmartScreen may warn, because the exe is unsigned: "More info" > "Run anyway".
4. Drag your game folder onto the window, or click the folder button on the right.
5. Press **Play**.

The first time, the launcher translates the game's shaders before starting, with a progress bar. The first session is
slower while it writes down the pipelines the game uses. On the next start, the launcher compiles those pipelines ahead
(2–3 minutes), and from then on the game loads completely.

## Requirements

- Windows 11 (Windows 10 was not tested) and a DirectX 12 graphics card.
  - 12 GB of VRAM or more: the default.
  - 8 GB: choose "8 GB" in the launcher (smaller textures).
- 32 GB of RAM recommended. The game commits 15–22 GB on top of what other programs use. See [Current focus](#current-focus).
- If `VCRUNTIME140.dll` is missing, install Microsoft's Visual C++ Redistributable (x64).

## The launcher (`opti-vicity.exe`)

| | |
|---|---|
| **Game folder** | Drag it onto the window, or use the folder button. The other button shows the game in Explorer. |
| **Series X** | The bigger console: sharper and with a larger texture budget, but slower. Series S is the default. |
| **Performance overlay** | On by default; F10 shows or hides it in the game. |
| **Graphics card** | "12 GB or more" (full textures) or "8 GB" (the top texture level dropped). |
| **Mouse** | Mouse look speed, or off. |
| **Upscaler** | The game's own (its console FSR, which takes the Quality size only), AMD FSR 3.1 for PC (any card; shipped in `fidelityfx`) or NVIDIA DLSS (RTX cards, with NVIDIA's files). |
| **Resolution** | The scene's size against the picture's: Quality 67%, Balanced 59%, Performance 50%, Ultra performance 33% (Native is not offered: at full size the game stops while making its first scene target). On this title the frame is mostly CPU work, so a smaller scene helps less than on most games. |
| **Frame gen** | Off, AMD FSR 3 (2x; picks AMD FSR 3.1 as the upscaler) or DLSS 2x to 6x (RTX 40/50; 3x and more need an RTX 50). |
| **Get the DLSS files** | Downloads NVIDIA's Streamline SDK (~263 MB, once) from NVIDIA's GitHub with Windows' `curl.exe` and keeps its DLSS and DLSS frame generation files in `nvidia-sdk` (`tar.exe` unpacks them). No script is involved: a download script in the package made Defender call the zip a trojan. They are under NVIDIA's licences and do not come with opti-vicity. |
| **Prepare shaders** | Translates the shaders and compiles the pipelines ahead without starting the game. |
| **Play** | Prepares first when this build needs it, then starts the game. The launcher minimizes and comes back when the game closes. |
| **Logs** | Opens the folder with `game.log` and `prepare.log`. Send them along with a bug report. |

Command line: `opti-vicity.exe [--game <folder>] [--play]`.

What it keeps where:

- **Choices:** `%LOCALAPPDATA%\opti-vicity\launcher.ini`.
- **Cache** (shaders and pipelines): `%LOCALAPPDATA%\opti-vicity\cache`.
- **Logs:** `%LOCALAPPDATA%\opti-vicity\logs`.
- **Saves:** they stay where every VIctory build keeps them, in `%LOCALAPPDATA%\victory\data`.

The engine is `victory.exe` in `bin\`. The launcher starts it with the settings in
[`apps/opti-vicity/src/settings.rs`](apps/opti-vicity/src/settings.rs), and reads the progress of
`victory shaders` / `victory warm --used` from the `progress <n> of <total>` lines they print with
`VICTORY_PROGRESS=1`.

## Controls

The title only knows controllers, so the keyboard and mouse drive a stand-in controller. A real controller works at the
same time.

| Key / mouse | Controller |
|---|---|
| **Click on the picture**, or **F9** | Take the mouse for looking around (the pointer is hidden). |
| **F9**, or Alt+Tab | Let the pointer go. |
| Mouse movement | Right stick. |
| Left / right mouse button | Right / left trigger (shoot / aim). |
| W A S D | Left stick. |
| I J K L | Right stick (keys). |
| Space or Enter | A |
| Backspace | B |
| X / Y | X / Y |
| Q / E | Left / right bumper. |
| Shift / Ctrl (or F / H) | Left / right stick click. |
| Z / C | Left / right trigger (keys). |
| Esc / Tab | Menu / View. |
| Arrows | D-pad. |
| F1 | The options menu (arrows pick and change, Enter confirms, F1 or Esc closes). The game gets no keys or mouse while it is open; a controller still works. |
| F10 | Show or hide the overlay. |

The mouse works like a stick, not a direct camera: about 1,000 pixels a second pushes the stick to its end, and the
movement is smoothed over a few readings.

## What opti-vicity adds

The base is [Yaekai/victory-scarlett-3070](https://github.com/Yaekai/victory-scarlett-3070) as of 2026-11-26. On top of
it:

- **Launcher** (`apps/opti-vicity`): the Win32 window above.
  - Its icon is drawn in `build.rs`, so the repository keeps no binary file.
  - The progress lines (`VICTORY_PROGRESS`) were added to `apps/victory/src/translate.rs` and `warm.rs`.
- **Settings** (`apps/opti-vicity/src/settings.rs`): Yaekai's `scripts/play-3070.ps1` set, with these changes:

  | Setting | play-3070 | opti-vicity | Why |
  |---|---|---|---|
  | `VICTORY_TEXTURE_MIP_DROP` | 1 | **0** (1 with "8 GB") | Full textures on 12 GB+ cards. |
  | `VICTORY_PIPELINE_WAIT_MS` | 20 | **2000** | With 20 ms, a draw whose pipeline was still compiling was skipped. On a first visit that meant black screens, broken text and people without heads. |
  | `VICTORY_CACHE_DIR` | default | `%LOCALAPPDATA%\opti-vicity\cache` | One fixed cache. Pipelines are written down (`VICTORY_PIPELINE_BOOK=1`) and compiled ahead after each new build (`victory warm --used`). |
  | `VICTORY_LOG` | info | warn | Smaller logs. |

- **Performance overlay** (`crates/victory-gpu/src/overlay.rs`, `pixels.rs`): FPS and ms with the 30 s high/low/average,
  draws, CPU and GPU %, RAM, VRAM, the hardware and the build.
  - It is a separate click-through window drawn with GDI on its own thread, so it costs the frame nothing.
  - F10 shows or hides it; `VICTORY_OVERLAY=0` turns it off.
- **Mouse and keyboard** (`crates/victory-hle/src/modules/gameinput.rs`):
  - the mouse on the stand-in pad;
  - Space as A, and Shift/Ctrl as stick clicks;
  - `VICTORY_MOUSE` scales the mouse, and 0 turns it off.
- **Build label** (`crates/victory-gpu/build.rs`): it follows the current commit.
- **Upscaling and frame generation:**
  - **AMD FSR 3.1 for PC** (`crates/victory-hle/src/modules/fidelityfx.rs`): serves the title's FidelityFX calls, at any render
    scale (`apps/victory/src/render_scale.rs` patches the title's render size, after trustno1's VIctory-NVIDIA).
  - **AMD FSR 3 frame generation** (`crates/victory-gpu/src/framegen.rs`, after gleaf-6's VIctory-7900XTX): AMD's frame
    generation swap chain, fed by each upscaling. With FSR it is always made ready, so the F1 menu can switch it at once.
  - **NVIDIA DLSS and DLSS frame generation** (`crates/victory-gpu/src/streamline.rs`, `crates/victory-hle/src/modules/dlss.rs`):
    trustno1's VIctory-NVIDIA port through NVIDIA Streamline. DLSS frame generation needs an RTX 40/50 and hardware GPU
    scheduling; where it is missing it turns itself off and DLSS upscaling still runs.
  - Switches: `VICTORY_FIDELITYFX_DIR`, `VICTORY_RENDER_SCALE`, `VICTORY_FRAMEGEN=amd|dlss`, `VICTORY_FRAMEGEN_START`,
    `VICTORY_DLSS`, `VICTORY_DLSS_MODE`, `VICTORY_STREAMLINE_DIR`, `VICTORY_FRAMEGEN_X`. The launcher sets them.
- **F1 options menu** (`crates/victory-gpu/src/menu.rs`, drawn by the overlay thread), after VIctory-NVIDIA's: frame
  generation (live), upscaler, render scale, console and the overlay; saved into `launcher.ini`, and "save and restart"
  ends the game with code 82 for the launcher to start it again.
- **Open water without dark blocks on NVIDIA** (`crates/victory-gpu/src/pipeline/make.rs`, from VIctory-NVIDIA 35c1560):
  the water's two tiled light programs leave whole tiles of the sea dark on 32-lane cards and are left out there
  (darker, bluer water; `VICTORY_WATER_LIGHT=1` runs them).
- **Crash notice in the launcher** (`apps/opti-vicity/src/crash.rs`): when the game closes with an error, the launcher
  reads `game.log` and shows the cause, what to try and the log's lines (exception and stack, engine error, lost device,
  full paging file, memory refused with `0x8007000E`); Ctrl+C copies it.
- **AMD shader compiler crash** (`crates/victory-shader/src/emit/bitcode/opt.rs`, from RAZDA4A-XEN 453597e): a
  `getDimensions` query is no longer shared between uses, which ended the process in AMD's compiler
  (`amdxc64.dll+0x861b9f`).
- **A pipeline that stops the driver is left out next time** (`crates/victory-core/src/doing.rs`,
  `crates/victory-gpu/src/pipeline/host.rs`): the crash report names the pipeline being made ("while making the host
  pipeline of ..."), the launcher writes its programs into `leave-out.txt` in the cache, and the engine never makes them
  again (`VICTORY_LEAVE_OUT`, any build). Deleting the file brings them back.

## Build from source

1. Install [Rust](https://rustup.rs) and the Visual Studio 2022 C++ build tools with a Windows SDK. The SDK's `rc.exe`
   embeds the launcher's icon and manifest.
2. Build:

   ```powershell
   cargo build --release --locked -p victory -p opti-vicity
   ```

3. Run `target\release\opti-vicity.exe`. It finds `victory.exe` next to itself.

For a release, put `opti-vicity.exe` at the top of a folder and `victory.exe` in its `bin\`.

## Current focus

The next goal is **asset loading on scene changes**. Today, not everything is in place in time, and that causes
hitches. The logs point at host memory:

- **When the brake starts:** the engine enters a "host memory pressure" mode when Windows has **4 GB or less** of commit
  available (RAM plus pagefile). It only leaves that mode at 6 GB.
- **What the brake does:** it caps the textures uploaded per frame at 64 MB, or at 16 MB below 1.5 GB of commit.
- **In a 5-minute test:** pressure came on after about a minute and never cleared, while 9 GB of physical RAM was still
  free. Texture loading was cut 7,424 times at the 16 MB level.
- **Why commit runs out:** the emulated console memory, the GPU-side copies and the staging buffers duplicate data that
  the console keeps only once, and the engine reserves memory up front.

Work in progress:

- measuring what fills that memory;
- reserving it on demand instead of up front;
- making the brake follow physical RAM and favour the textures of the scene that is coming in.

## Measurements (one PC, for reference)

**Test PC:**
- Ryzen 7 5800X3D, RTX 3080 Ti 12 GB, 32 GB RAM, Windows 11.
- Resizable BAR off.

**Method:**
- Every run used the same save, restored fresh each time, in the prison area.
- Each measured run lasted 5 minutes, after a warm-up run.
- Input was scripted to get past the menus.
- Results are the median ms per frame. Because the scene drifts between runs, they are also compared by draw count.

| Build / settings | Median ms | Draws (median) | Notes |
|---|---:|---:|---|
| **opti-vicity (Series S, full textures)** | **54.6** | 3,846 | CPU preparation 36 ms/frame. The picture was correct. |
| cool_fox (prebuilt) + `LEVELS_ON_DEMAND=0`, `CARD_BUFFERS_MB=1024`, `CARD_WINDOWS_MB=256` | 58.6 | 5,415 | CPU preparation 46 ms/frame. |
| cool_fox defaults (pareto-residency profile) | ~505 | n/a | GPU-bound, about 2 fps. |
| Op-lite/victory-scarlett, its own launcher (Series X) | 288.7 | n/a | |
| gleaf-6/VIctory-7900XTX- with the cool_fox settings above | 76.6 | 4,499 | |
| Dolberthi/neptune-dolberth-nvidia-8gb, run-neptune.ps1 | 84 | 4,491 | Different spot in the scene (the auto-input differed). |
| rorodev6/victory-nvidia-megafork, play-nvidia.ps1 (540p) | 81–92 | 3,446 | |

By draw count, opti-vicity against the tuned cool_fox (ms per frame):

| Draws per frame | opti-vicity | cool_fox tuned |
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
  - **`VICTORY_LAND_THREADS=8`** gained about 1 ms but ran out of texture room, which made the picture dark and ghosted.

## Credits

opti-vicity stands on these projects. Their READMEs are kept in
[`docs/upstream-readmes.md`](docs/upstream-readmes.md).

| Project | What comes from it |
|---|---|
| [Marvisak/VIctory](https://github.com/Marvisak/VIctory) | The original translation layer: running the title's code and translating its GPU work and shaders to Direct3D 12. |
| [fxceless/victory-fork](https://github.com/fxceless/victory-fork) | The community fork most later work is based on. |
| codec-d/cool_fox | The residency and copy work (`Play-VIctory` profiles) that scarlett and Yaekai continue. |
| [Op-lite/victory-scarlett](https://github.com/Op-lite/victory-scarlett) | The "Scarlett" CPU-submit, upload and texture-loading work. |
| [Yaekai/victory-scarlett-3070](https://github.com/Yaekai/victory-scarlett-3070) | The base of opti-vicity: NVIDIA heap fixes, visual fixes, the 60 fps unlock and the `play-3070` settings. |
| [Dolberthi/neptune-dolberth-nvidia-8gb](https://github.com/Dolberthi/neptune-dolberth-nvidia-8gb) | The performance overlay. The overlay itself is by zm0kie. |
| [zm0kiemedia/Neptune_VI](https://github.com/zm0kiemedia/Neptune_VI) | Mouse look and mouse buttons on the stand-in controller. |
| [trustno1-coder/VIctory-NVIDIA](https://github.com/trustno1-coder/VIctory-NVIDIA) | DLSS and DLSS frame generation through NVIDIA Streamline, the render-scale patch, the F1 settings menu it inspired, the way of fetching NVIDIA's files and the open-water fix. |
| [gleaf-6/VIctory-7900XTX-](https://github.com/gleaf-6/VIctory-7900XTX-) | AMD FSR 3 frame generation on the host. |
| [crossducking/RAZDA4A-XEN](https://github.com/crossducking/RAZDA4A-XEN) | The fix for AMD's shader compiler crash (`getDimensions`). |

The git history keeps the line: `ee36438` imports Yaekai's master as it was on 2026-11-26, and the commits after it are
opti-vicity's.

## License

GPL-3.0-or-later (see [LICENSE](LICENSE)), as for every project above. A release ships the source for that exact build
as this repository at the commit named in the release.

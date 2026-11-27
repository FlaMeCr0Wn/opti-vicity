# Starts the title with this fork's settings: Yaekai's play-3070 set (Series S), with full-size textures for
# 12 GB+ cards and the first-run fixes, plus the performance overlay (F10 shows/hides it).
#
#   .\scripts\play-unificado.ps1 -Game 'D:\Games\MyDump'   first time: the folder with game_scarlett_master.exe
#   .\scripts\play-unificado.ps1                           later: the folder is remembered in game-folder.txt
#   -SeriesX      the bigger console (sharper, larger texture budget, slower)
#   -MipDrop 1    drop the top texture level (for 8 GB cards; 0 = full textures, the default)
#   -NoOverlay    no overlay (same as VICTORY_OVERLAY=0)
#   -MouseSpeed   mouse look speed (VICTORY_MOUSE; default 1, 0 = mouse off)
#   -Prepare     only translate the shaders and compile the pipelines ahead, then quit
#   -CacheDir     where shaders/pipelines are kept (default %LOCALAPPDATA%\victory\cache-unificado)
#   -More         other settings, last word: -More @{ VICTORY_LOG = 'info' }
#
# First runs: the first time a pipeline is met it has to be compiled. This launcher keeps one cache folder, writes
# down every pipeline the game uses (VICTORY_PIPELINE_BOOK), and after a new build compiles them ahead
# (`victory shaders` + `victory warm --used`). Draws also wait up to 2 s for a pipeline still compiling instead of
# being skipped (the play-3070 value of 20 ms gave black screens, broken text and headless people on first visits).
# So the very first session is slower while it fills the cache; later ones are not.
#
# Mouse and keyboard: click on the picture (or F9) to look around with the mouse; F9 or Alt+Tab lets the pointer go.
# WASD move, left/right mouse button = right/left trigger, Space or Enter = A, Backspace = B, Q/E bumpers,
# Shift/Ctrl stick clicks, Esc menu, Tab view, arrows D-pad (the README has the whole table).
param(
    [string]$Game,
    [switch]$SeriesX,
    [ValidateRange(0, 3)][int]$MipDrop = 0,
    [switch]$NoOverlay,
    [ValidateRange(0.0, 10.0)][double]$MouseSpeed = 1.0,
    [switch]$Prepare,
    [string]$CacheDir = (Join-Path $env:LOCALAPPDATA 'victory\cache-unificado'),
    [hashtable]$More = @{}
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot

# The build: from source (target\release) or a prebuilt one next to the README / in bin\.
$exe = @('target\release\victory.exe', 'bin\victory.exe', 'victory.exe') |
    ForEach-Object { Join-Path $root $_ } | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf } | Select-Object -First 1
if (-not $exe) { throw 'No victory.exe. Build it first: cargo build --release --locked -p victory' }

# The game folder, remembered.
$remembered = Join-Path $root 'game-folder.txt'
if (-not $Game -and (Test-Path -LiteralPath $remembered)) { $Game = (Get-Content -LiteralPath $remembered -TotalCount 1) }
if (-not $Game) { throw "Say where your dump is: .\scripts\play-unificado.ps1 -Game 'D:\Games\MyDump' (or drag the folder onto Play-Unificado.cmd)" }
$Game = $Game.Trim().Trim('"')
if ((Test-Path -LiteralPath $Game -PathType Leaf) -and $Game.EndsWith('.exe')) { $Game = Split-Path -Parent $Game }
$title = Join-Path $Game 'game_scarlett_master.exe'
if (-not (Test-Path -LiteralPath $title -PathType Leaf)) { throw "No game_scarlett_master.exe in $Game. -Game is the folder of your dump." }
Set-Content -LiteralPath $remembered -Value $Game -Encoding utf8
if (Get-Process victory -ErrorAction SilentlyContinue) { Write-Host 'victory.exe is already running.' -ForegroundColor Yellow; exit 1 }

# Every setting comes from here; nothing left over in this window counts (saves stay in the default place).
Get-ChildItem Env:VICTORY_* -ErrorAction SilentlyContinue | Where-Object { $_.Name -ne 'VICTORY_DATA_DIR' } |
    ForEach-Object { Remove-Item "Env:$($_.Name)" }
$set = [ordered]@{
    VICTORY_GAME_DIR             = $Game
    VICTORY_CACHE_DIR            = $CacheDir
    VICTORY_LOG                  = 'warn'
    VICTORY_PIPELINE_BOOK        = '1'
    VICTORY_FRAME_INTERVAL_MS    = '16.667'
    # scripts\play-3070.ps1 as it is, except MipDrop (0 here) and PIPELINE_WAIT_MS (below).
    VICTORY_DYNAMIC_SLOTS        = '524288'
    VICTORY_MAX_PRELUDES         = '32'
    VICTORY_CARD_BUFFERS_MB      = '1024'
    VICTORY_CARD_WINDOWS_MB      = '512'
    VICTORY_TEXTURE_MIP_DROP     = "$MipDrop"
    VICTORY_TEXTURE_ROOM_PERCENT = '94'
    VICTORY_PAGE_POOL            = '1'
    VICTORY_STAGING_PACK         = '1'
    VICTORY_LEVELS_ON_DEMAND     = '0'
    VICTORY_WAIT_RESIDENT = '0'; VICTORY_JOIN_COPIES = '0'; VICTORY_REFRESH_ONCE = '1'; VICTORY_CARD_REHASH_RUNS = '1'
    VICTORY_BACK_OWN_VIEW = '1'; VICTORY_WIDE_MERGE = '1'; VICTORY_MIRROR_FAST = '1'
    VICTORY_ASYNC_SUBMIT = '1'; VICTORY_QUEUE2 = '1'; VICTORY_INDIRECT_WINDOW = '1'; VICTORY_TEXTURE_NO_WAIT = '1'
    VICTORY_COPY_PLAIN = '1'; VICTORY_WIDE_TEXTURES = '1'; VICTORY_INDIRECT_GATHER = '1'; VICTORY_LATE_WAIT_LISTS = '1024'
    VICTORY_CARD_FILLS_BATCH = '0'; VICTORY_GATHER_DISPATCH = '0'; VICTORY_RELAY = '2'; VICTORY_WIDE_BACK = '1'
    VICTORY_RELAY_COPY = '1'; VICTORY_CARD_STEPS = '2'; VICTORY_INDIRECT_BATCH = '2'; VICTORY_MAP_THREADS = '1'
    VICTORY_CARD_AWAKE = '0'; VICTORY_SHOW_AFTER = '1'; VICTORY_JOIN_PARTS = '8'; VICTORY_UPLOAD_AHEAD = '1'
    VICTORY_CARD_BUFFERS_MIN_KB = '64'; VICTORY_POOL_DISCARD = '1'; VICTORY_FILL_TITLE = '1'; VICTORY_PARTIAL_DELTA = '1'
    VICTORY_BAKE_DELTA = '1'; VICTORY_MAP_LOCK_FINE = '1'; VICTORY_POOL_CAP = '1'; VICTORY_TABLE_MEMO = '1'
    VICTORY_LIST_ALIAS_EXACT = '1'; VICTORY_LIST_HOLD = '1'
    VICTORY_EARLY_TEXTURES_MS    = '40'
    # The title's character/decoration bake waits up to 10 s for its textures (0 leaves them missing).
    VICTORY_BAKE_TEXTURES_MS     = '10000'
    VICTORY_HEAP_PASS_MAX = '0'; VICTORY_HEAP_CHANGED_MAX = '0'
    VICTORY_SKIP_PIPELINES       = '02384a60fda6f2d0,731a39171b568298'
    VICTORY_WC_NO_BACK = '1'; VICTORY_WC_NO_BACK_MIN_KB = '1024'; VICTORY_SYNC_IN_PLACE = '1'; VICTORY_SPANS_QUICK = '1'
    VICTORY_SYNC_FINE = '1'; VICTORY_HEAP_SYNC_FIRST = '1'
    # No ray tracing: the stand-in ambient light.
    VICTORY_AMBIENT = '0.8'; VICTORY_AMBIENT_AIR = '1.5'
    # First visits: wait for a compiling pipeline instead of skipping the draw (see the top).
    VICTORY_PIPELINE_WAIT_MS     = '2000'
}
if ($NoOverlay) { $set.VICTORY_OVERLAY = '0' }
$set.VICTORY_MOUSE = $MouseSpeed.ToString([System.Globalization.CultureInfo]::InvariantCulture)
foreach ($k in $More.Keys) { $set[$k] = "$($More[$k])" }
foreach ($k in $set.Keys) { Set-Item "Env:$k" $set[$k] }

# After a new build (or the first time): shaders translated and the used pipelines compiled ahead.
$item = Get-Item -LiteralPath $exe
$want = "$($item.Length):$($item.LastWriteTimeUtc.Ticks)"
$stamp = Join-Path $CacheDir 'unificado-prepared.txt'
$have = if (Test-Path -LiteralPath $stamp) { (Get-Content -LiteralPath $stamp -Raw).Trim() } else { '' }
if ($Prepare -or $have -ne $want) {
    Write-Host 'Preparing this build (shaders, then pipelines ahead): a few minutes, once per build...' -ForegroundColor Cyan
    New-Item -ItemType Directory -Force $CacheDir | Out-Null
    $ErrorActionPreference = 'Continue'
    $rpf = Join-Path $Game 'shaders.rpf'
    $ok = $true
    if (Test-Path -LiteralPath $rpf -PathType Leaf) {
        & $exe shaders $rpf
        $ok = $LASTEXITCODE -eq 0
    } else {
        Write-Warning "No shaders.rpf in ${Game}: the shaders will be translated while playing."
    }
    if (Test-Path -LiteralPath (Join-Path $CacheDir 'pipelines\known-pipelines.bin')) {
        & $exe warm --used
        $ok = $ok -and $LASTEXITCODE -eq 0
    } else {
        Write-Host 'No pipelines written down yet: this first session writes them (slower while it does).'
        # Leave the stamp unwritten so the next start compiles what this session wrote down.
        $ok = $false
    }
    $ErrorActionPreference = 'Stop'
    if ($ok) { Set-Content -LiteralPath $stamp -Value $want -NoNewline }
    if ($Prepare) { exit 0 }
}

$console = if ($SeriesX) { 'series-x' } else { 'series-s' }
Write-Host "$console, textures mip drop $MipDrop, overlay $(if ($NoOverlay) { 'off' } else { 'on (F10)' })"
Write-Host "Build: $exe"
Write-Host "Cache: $CacheDir"
& $exe run $title --execute --console $console
exit $LASTEXITCODE

# ACC-ENGINE-LLAMACPP-452: fetch the pinned llama.cpp runtime (Vulkan build, b11207) and the
# Qwen3-ASR 1.7B GGUF model (Q8_0 + Q8_0 mmproj), verify sha256, and sync them next to the exe.
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File scripts\fetch-llama-runtime.ps1
#   powershell -ExecutionPolicy Bypass -File scripts\fetch-llama-runtime.ps1 -Targets target\release,Publish
#
# Runtime DLLs go next to feiyin-ime.exe (ggml picks Vulkan / the best CPU variant at run time).
# The model goes to <models>\qwen3-asr-1.7b-gguf\ (target\release\models is a link to .\models).

param(
    [string[]]$Targets = @("target\debug", "target\release", "Publish"),
    [switch]$SkipModel
)
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
# "powershell -File ... -Targets a,b" passes one string "a,b"; split it.
$Targets = @($Targets | ForEach-Object { $_ -split "," } | Where-Object { $_ -ne "" })

$Tag = "b11207"
$Zip = "llama-$Tag-bin-win-vulkan-x64.zip"
$ZipUrl = "https://github.com/ggml-org/llama.cpp/releases/download/$Tag/$Zip"
$ZipSha = "fc1018a0e83071598268338ed7ad422d176dd65aa86780ba69530f4a658a37af"

# Only what the engine needs (no tools, no rpc, no llama-common).
$Dlls = @(
    "ggml-base.dll", "ggml.dll", "llama.dll", "mtmd.dll", "ggml-vulkan.dll", "libomp.dll",
    "ggml-cpu-alderlake.dll", "ggml-cpu-cannonlake.dll", "ggml-cpu-cascadelake.dll",
    "ggml-cpu-cooperlake.dll", "ggml-cpu-haswell.dll", "ggml-cpu-icelake.dll",
    "ggml-cpu-ivybridge.dll", "ggml-cpu-piledriver.dll", "ggml-cpu-sandybridge.dll",
    "ggml-cpu-sapphirerapids.dll", "ggml-cpu-skylakex.dll", "ggml-cpu-sse42.dll",
    "ggml-cpu-x64.dll", "ggml-cpu-zen4.dll"
)

$ModelSub = "qwen3-asr-1.7b-gguf"
$HfBase = "https://huggingface.co/ggml-org/Qwen3-ASR-1.7B-GGUF/resolve/main"
$Models = @(
    @{ Name = "Qwen3-ASR-1.7B-Q8_0.gguf"; Sha = "58e22d0532d4eacaf034cfac17a6fed159f37c41390c710186783be439d1fc57" },
    # MEM-453: audio encoder Q8_0 (was f16).
    @{ Name = "mmproj-Qwen3-ASR-1.7B-Q8_0.gguf"; Sha = "46c1d533af3f354ceb37ce855dbceff7da7fa7cf1e6a523df3b13440bd164c0d" }
)

function Get-Sha([string]$Path) { (Get-FileHash -Algorithm SHA256 -Path $Path).Hash.ToLower() }

function Fetch([string]$Url, [string]$Dest, [string]$Sha) {
    if ((Test-Path $Dest) -and ((Get-Sha $Dest) -eq $Sha)) { return }
    Write-Host "downloading $Url"
    & curl.exe -sSL -o $Dest $Url
    if ($LASTEXITCODE -ne 0) { throw "download failed: $Url" }
    $got = Get-Sha $Dest
    if ($got -ne $Sha) { throw "sha256 mismatch for $Dest ($got)" }
}

# ---- runtime ----
$Vendor = Join-Path $Root "vendor\llama-runtime"
$Extract = Join-Path $Vendor $Tag
New-Item -ItemType Directory -Force -Path $Vendor | Out-Null
$ZipPath = Join-Path $Vendor $Zip
Fetch $ZipUrl $ZipPath $ZipSha
if (-not (Test-Path (Join-Path $Extract "llama.dll"))) {
    Expand-Archive -Path $ZipPath -DestinationPath $Extract -Force
}
foreach ($t in $Targets) {
    $dst = Join-Path $Root $t
    if (-not (Test-Path $dst)) { continue }
    foreach ($d in $Dlls) { Copy-Item -Force (Join-Path $Extract $d) $dst }
    Write-Host "runtime -> $dst ($($Dlls.Count) dll)"
}

# ---- model ----
if (-not $SkipModel) {
    $mdir = Join-Path $Root "models\$ModelSub"
    New-Item -ItemType Directory -Force -Path $mdir | Out-Null
    foreach ($m in $Models) { Fetch "$HfBase/$($m.Name)" (Join-Path $mdir $m.Name) $m.Sha }
    # tokenizer.json is only used to count wordbook tokens (same Qwen3 tokenizer as the GGUF vocab;
    # byte-identical to Qwen/Qwen3-0.6B tokenizer.json, verified 2026-09-27).
    $tok = Join-Path $mdir "tokenizer"
    New-Item -ItemType Directory -Force -Path $tok | Out-Null
    Fetch "https://huggingface.co/Qwen/Qwen3-0.6B/resolve/main/tokenizer.json" (Join-Path $tok "tokenizer.json") "aeb13307a71acd8fe81861d94ad54ab689df773318809eed3cbe794b4492dae4"
    Write-Host "model -> $mdir"
    $pub = Join-Path $Root "Publish\models"
    if ((Test-Path $pub) -and -not ((Get-Item $pub).Attributes -band [IO.FileAttributes]::ReparsePoint)) {
        $pdir = Join-Path $pub $ModelSub
        New-Item -ItemType Directory -Force -Path $pdir | Out-Null
        foreach ($m in $Models) { Copy-Item -Force (Join-Path $mdir $m.Name) $pdir }
        Copy-Item -Recurse -Force $tok $pdir
        Write-Host "model -> $pdir"
    }

    # ---- FORCED-ALIGN-456: Qwen3-ForcedAligner-0.6B (bundled with the 1.7B model for local realtime) ----
    # Source: 42ailab/Qwen3-ForcedAligner-0.6B-GGUF (Apache-2.0; f16 backbone + f16 audio encoder + head.bin).
    # We ship Q8_0 of both GGUFs, rebuilt here from the public f16 files and pinned by sha256:
    #   backbone: llama-quantize (same b11207 runtime) Q8_0
    #   encoder : crispasr-quantize (CrispASR v0.8.38) q8_0, official ggml-org recipe
    #             (position_embd stays f32, conv_out stays f16) - llama-quantize cannot quantize mmproj files.
    $aSub = "qwen3-forcedaligner-0.6b-gguf"
    $aBase = "https://huggingface.co/42ailab/Qwen3-ForcedAligner-0.6B-GGUF/resolve/main"
    $aDir = Join-Path $Root "models\$aSub"
    New-Item -ItemType Directory -Force -Path $aDir | Out-Null
    $aOut = @(
        @{ Name = "aligner-backbone-q8_0.gguf"; Sha = "1b5ea4c29aef7534798bb58f54ab9ae10c77bab6e92e59142158b001f319d97b" },
        @{ Name = "aligner-mmproj-q8_0.gguf"; Sha = "7117f45d7d87b4714304bb82d0c43b28cec98287b81c88259a235acc347350d3" },
        @{ Name = "aligner-head.bin"; Sha = "8a3f5ed737b36838f078e585b8844d24a1492cf18a8a32c51092f384fc706d28" }
    )
    $aReady = $true
    foreach ($m in $aOut) { $p = Join-Path $aDir $m.Name; if (-not ((Test-Path $p) -and ((Get-Sha $p) -eq $m.Sha))) { $aReady = $false } }
    if (-not $aReady) {
        $work = Join-Path $Vendor "aligner-src"
        New-Item -ItemType Directory -Force -Path $work | Out-Null
        Fetch "$aBase/aligner-head.bin" (Join-Path $aDir "aligner-head.bin") $aOut[2].Sha
        $bb = Join-Path $work "aligner-backbone-f16.gguf"
        $mp = Join-Path $work "aligner-mmproj-f16.gguf"
        Fetch "$aBase/aligner-backbone-f16.gguf" $bb "ceff10e4b524bf0d683bce032e035aaaeaaa265537b55d4e2e1867601638cefd"
        Fetch "$aBase/aligner-mmproj-f16.gguf" $mp "73559b9e10132bdc97e001d5d383e6e72ebaf043fda4c4fab45c82ddbe0f2d67"
        & (Join-Path $Extract "llama-quantize.exe") $bb (Join-Path $aDir "aligner-backbone-q8_0.gguf") Q8_0 | Out-Null
        if ($LASTEXITCODE -ne 0) { throw "llama-quantize failed" }
        $cz = Join-Path $Vendor "crispasr-windows-x86_64-vulkan-v0.8.38.zip"
        Fetch "https://github.com/CrispStrobe/CrispASR/releases/download/v0.8.38/crispasr-windows-x86_64-vulkan.zip" $cz "cae3f60051379eebf64330accc7c706b65eba169705b85b90d3fcd49f049fcab"
        $cx = Join-Path $Vendor "crispasr-v0.8.38"
        if (-not (Test-Path $cx)) { Expand-Archive -Path $cz -DestinationPath $cx -Force }
        $cq = Get-ChildItem -Path $cx -Recurse -Filter "crispasr-quantize.exe" | Select-Object -First 1
        & $cq.FullName $mp (Join-Path $aDir "aligner-mmproj-q8_0.gguf") q8_0 --tensor-type 'position_embd=f32' --tensor-type 'conv_out=f16' | Out-Null
        if ($LASTEXITCODE -ne 0) { throw "crispasr-quantize failed" }
        foreach ($m in $aOut) {
            $got = Get-Sha (Join-Path $aDir $m.Name)
            if ($got -ne $m.Sha) { throw "aligner sha256 mismatch for $($m.Name) ($got)" }
        }
    }
    Write-Host "aligner -> $aDir"
    if ((Test-Path $pub) -and -not ((Get-Item $pub).Attributes -band [IO.FileAttributes]::ReparsePoint)) {
        $pa = Join-Path $pub $aSub
        New-Item -ItemType Directory -Force -Path $pa | Out-Null
        foreach ($m in $aOut) { Copy-Item -Force (Join-Path $aDir $m.Name) $pa }
        Write-Host "aligner -> $pa"
    }
}

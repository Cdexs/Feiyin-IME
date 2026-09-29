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
}

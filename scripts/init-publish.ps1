# init-publish.ps1
# One-time setup: initialize Publish/ and target/release/ with external dependencies
# Run after: cargo build --release (DLLs must exist in target/release/)

param(
    [switch]$SkipModels   # Skip model junction creation
)

$ProjectRoot = Split-Path $PSScriptRoot -Parent
$TargetRelease = Join-Path $ProjectRoot "target\release"
$Publish = Join-Path $ProjectRoot "Publish"
$Models = Join-Path $ProjectRoot "models"

Write-Host "=== voice-ime init-publish ===" -ForegroundColor Cyan
Write-Host "Project root: $ProjectRoot"
Write-Host "Publish dir:  $Publish"

if (-not (Test-Path $Publish)) { New-Item -ItemType Directory -Path $Publish | Out-Null }

# Step 1: Copy DLLs to Publish/
Write-Host "`n[Step 1] Copy DLLs to Publish/" -ForegroundColor Yellow
$dlls = @(
    "sherpa-onnx-c-api.dll",
    "sherpa-onnx-cxx-api.dll",
    "onnxruntime.dll",
    "onnxruntime_providers_shared.dll",
    "ctranslate2.dll",
    "libiomp5md.dll",
    "cudnn64_9.dll"        # optional: GPU inference
)
foreach ($dll in $dlls) {
    $src = Join-Path $TargetRelease $dll
    $dst = Join-Path $Publish $dll
    if (Test-Path $src) {
        Copy-Item -Path $src -Destination $dst -Force
        Write-Host "  OK $dll"
    } else {
        Write-Host "  MISSING $dll (run cargo build --release first)" -ForegroundColor Red
    }
}

# Step 2: Copy default config template to Publish/ and target/release/
Write-Host "`n[Step 2] Copy default config template" -ForegroundColor Yellow
$defaultConfig = Join-Path $ProjectRoot "assets\default-config.toml"

$publishConfig = Join-Path $Publish "config.toml"
if (-not (Test-Path $publishConfig)) {
    Copy-Item -Path $defaultConfig -Destination $publishConfig -Force
    Write-Host "  OK Publish/config.toml"
} else {
    Write-Host "  SKIP Publish/config.toml (exists, preserving)"
}

$devConfig = Join-Path $TargetRelease "config.toml"
if (-not (Test-Path $devConfig)) {
    Copy-Item -Path $defaultConfig -Destination $devConfig -Force
    Write-Host "  OK target/release/config.toml"
} else {
    Write-Host "  SKIP target/release/config.toml (exists, preserving)"
}

$debugDir = Join-Path $ProjectRoot "target\debug"
if (Test-Path $debugDir) {
    $debugConfig = Join-Path $debugDir "config.toml"
    if (-not (Test-Path $debugConfig)) {
        Copy-Item -Path $defaultConfig -Destination $debugConfig -Force
        Write-Host "  OK target/debug/config.toml"
    } else {
        Write-Host "  SKIP target/debug/config.toml (exists, preserving)"
    }
}

# Step 3: Create models directory junctions (avoid 640MB+ copy)
Write-Host "`n[Step 3] Create models directory junctions" -ForegroundColor Yellow

if (-not (Test-Path $Models)) {
    Write-Host "  ERROR: models/ not found. Download models first." -ForegroundColor Red
} elseif ($SkipModels) {
    Write-Host "  SKIP (-SkipModels flag)"
} else {
    # Publish/models junction
    $publishModels = Join-Path $Publish "models"
    if (Test-Path $publishModels) {
        Write-Host "  SKIP Publish/models (exists)"
    } else {
        try {
            New-Item -ItemType Junction -Path $publishModels -Target $Models -ErrorAction Stop | Out-Null
            Write-Host "  OK Publish/models -> models/ (junction)"
        } catch {
            Write-Host "  WARN: junction failed, copying instead..." -ForegroundColor Yellow
            Copy-Item -Path $Models -Destination $publishModels -Recurse -Force
            Write-Host "  OK Publish/models (full copy)"
        }
    }

    # target/release/models junction
    $devModels = Join-Path $TargetRelease "models"
    if (Test-Path $devModels) {
        Write-Host "  SKIP target/release/models (exists)"
    } else {
        try {
            New-Item -ItemType Junction -Path $devModels -Target $Models -ErrorAction Stop | Out-Null
            Write-Host "  OK target/release/models -> models/ (junction)"
        } catch {
            Write-Host "  ERROR: Could not create target/release/models junction: $_" -ForegroundColor Red
        }
    }
}

# Step 4: Copy current EXEs to Publish/
Write-Host "`n[Step 4] Copy EXEs to Publish/" -ForegroundColor Yellow
$exes = @("voice-ime.exe", "voice-ime-ui.exe", "crash-reporter.exe")
foreach ($exe in $exes) {
    $src = Join-Path $TargetRelease $exe
    $dst = Join-Path $Publish $exe
    if (Test-Path $src) {
        Copy-Item -Path $src -Destination $dst -Force
        Write-Host "  OK $exe"
    } else {
        Write-Host "  SKIP $exe (not built yet)"
    }
}

Write-Host "`n=== Done ===" -ForegroundColor Green
Write-Host "Publish/ and target/release/ are ready for direct exe execution."
Write-Host "After each build, run build.bat to sync EXEs to Publish/ automatically."

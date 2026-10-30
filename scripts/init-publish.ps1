# init-publish.ps1
# One-time setup: initialize Publish/ and target/release/ with external dependencies
# Run after: cargo build --release (DLLs must exist in target/release/)

param(
    [switch]$SkipModels,  # FLASH-P3-A：无模型可链，参数保留只为旧调用方式不报错
    [switch]$RuntimeOnly  # Only run Step 2 (copy VC++ runtime DLLs) then exit — RELEASE-VCRT-APPLOCAL-399
)

$ProjectRoot = Split-Path $PSScriptRoot -Parent
$TargetRelease = Join-Path $ProjectRoot "target\release"
$Publish = Join-Path $ProjectRoot "Publish"

Write-Host "=== voice-ime init-publish ===" -ForegroundColor Cyan
Write-Host "Project root: $ProjectRoot"
Write-Host "Publish dir:  $Publish"

if (-not (Test-Path $Publish)) { New-Item -ItemType Directory -Path $Publish | Out-Null }

# Step 2 wrapped in a function so it can be run in isolation:
#   powershell -NoProfile -ExecutionPolicy Bypass -File scripts\init-publish.ps1 -RuntimeOnly
function Copy-VcRuntime {
    # TRANS / RELEASE-VCRT-APPLOCAL-399 + DEC-085：VC++ 2015-2022 x64 运行库随程序目录发布（app-local）。
    # 清单来源：Publish 下全部 exe/dll 的 `dumpbin /dependents` 汇总（只带实际被导入的运行库）：
    #   msvcp140 / msvcp140_1 / vcruntime140 / vcruntime140_1 / vcomp140（vcomp140 自 397 CT2 oneDNN 起引入）。
    # api-ms-win-crt-*（UCRT）属 Win10/11 系统组件，不随包；🔴 禁止从 System32 取（不可再分发）。
    $vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
    if (-not (Test-Path $vswhere)) {
        Write-Host "  ERROR: vswhere.exe not found: $vswhere" -ForegroundColor Red
        exit 1
    }
    $vsInstall = @(& $vswhere -latest -products * -property installationPath)[0]
    if ($vsInstall) { $vsInstall = $vsInstall.Trim() }
    if (-not $vsInstall) {
        Write-Host "  ERROR: no Visual Studio installation found via vswhere" -ForegroundColor Red
        exit 1
    }
    $redistRoot = Join-Path $vsInstall "VC\Redist\MSVC"
    if (-not (Test-Path $redistRoot)) {
        Write-Host "  ERROR: VC Redist root not found: $redistRoot" -ForegroundColor Red
        exit 1
    }
    # 取最新「数字版本」目录（忽略 v143 之类别名；它下面没有 x64/CRT），不写死版本号
    $verDir = Get-ChildItem -Path $redistRoot -Directory |
        Where-Object { $_.Name -match '^\d+(\.\d+)+$' } |
        Sort-Object { [version]$_.Name } -Descending |
        Select-Object -First 1
    if (-not $verDir) {
        Write-Host "  ERROR: no numeric version dir under $redistRoot" -ForegroundColor Red
        exit 1
    }
    $x64Dir = Join-Path $verDir.FullName "x64"
    $crtDir = Get-ChildItem -Path $x64Dir -Directory -Filter "Microsoft.VC14*.CRT" | Select-Object -First 1
    $ompDir = Get-ChildItem -Path $x64Dir -Directory -Filter "Microsoft.VC14*.OpenMP" | Select-Object -First 1
    if (-not $crtDir -or -not $ompDir) {
        Write-Host "  ERROR: Microsoft.VC14*.CRT / Microsoft.VC14*.OpenMP not found under $x64Dir" -ForegroundColor Red
        exit 1
    }

    # 同时放 Publish/ 与 target/release/：前者是直跑目录，后者是 .iss 的 Source 目录
    $vcRuntime = @(
        @{ Name = "msvcp140.dll";       Src = (Join-Path $crtDir.FullName "msvcp140.dll") },
        @{ Name = "msvcp140_1.dll";     Src = (Join-Path $crtDir.FullName "msvcp140_1.dll") },
        @{ Name = "vcruntime140.dll";   Src = (Join-Path $crtDir.FullName "vcruntime140.dll") },
        @{ Name = "vcruntime140_1.dll"; Src = (Join-Path $crtDir.FullName "vcruntime140_1.dll") },
        @{ Name = "vcomp140.dll";       Src = (Join-Path $ompDir.FullName "vcomp140.dll") }
    )
    foreach ($item in $vcRuntime) {
        if (-not (Test-Path $item.Src)) {
            Write-Host "  ERROR: missing runtime source $($item.Src)" -ForegroundColor Red
            exit 1
        }
        Copy-Item -Path $item.Src -Destination (Join-Path $Publish $item.Name) -Force
        Copy-Item -Path $item.Src -Destination (Join-Path $TargetRelease $item.Name) -Force
        Write-Host "  OK $($item.Name) <- $($item.Src)"
    }

    # FLASH-P3-A：原「ctranslate2.dll 必须在 Publish」检查随离线翻译（CT2）删除。
}

if ($RuntimeOnly) {
    Write-Host "`n[RuntimeOnly] Copy VC++ runtime DLLs (app-local)" -ForegroundColor Yellow
    Copy-VcRuntime
    Write-Host "`n=== Done (runtime DLLs only) ===" -ForegroundColor Green
    exit 0
}

# Step 1（FLASH-P3-A 删除）：原复制 sherpa-onnx / onnxruntime / CTranslate2 等模型运行库 DLL ——
# 输入法不再带任何本地模型，无 DLL 可复制。

# Step 2: Copy VC++ 2015-2022 x64 runtime DLLs app-local (DEC-085 / RELEASE-VCRT-APPLOCAL-399)
# 实现见上方 `function Copy-VcRuntime`；可用 `-RuntimeOnly` 单独运行本步。
Write-Host "`n[Step 2] Copy VC++ runtime DLLs (app-local)" -ForegroundColor Yellow
Copy-VcRuntime

# Step 3: Copy default config template to Publish/ and target/release/
Write-Host "`n[Step 3] Copy default config template" -ForegroundColor Yellow
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

# Step 4（FLASH-P3-A 删除）：原为 Publish/ 与 target/release/ 建 models 目录联接 —— 输入法不再带模型。

# Step 5: Copy current EXEs to Publish/
Write-Host "`n[Step 5] Copy EXEs to Publish/" -ForegroundColor Yellow
# RELEASE-ISS-FROM-PUBLISH-400/405：实际产物名为 feiyin-ime*.exe（旧 voice-ime.exe 已作废，见 .iss）。
# FLASH-P3-A：识别服务网关 feiyin-asr-gateway.exe 随本机识别服务删除。
$exes = @("feiyin-ime.exe", "feiyin-ime-ui.exe", "crash-reporter.exe")
foreach ($exe in $exes) {
    $src = Join-Path $TargetRelease $exe
    $dst = Join-Path $Publish $exe
    if (Test-Path $src) {
        Copy-Item -Path $src -Destination $dst -Force
        Write-Host "  OK $exe"
    } else {
        Write-Host "  ERROR: $exe missing (run cargo build --release first)" -ForegroundColor Red
        exit 1
    }
}

Write-Host "`n=== Done ===" -ForegroundColor Green
Write-Host "Publish/ and target/release/ are ready for direct exe execution."
Write-Host "After each build, run build.bat to sync EXEs to Publish/ automatically."

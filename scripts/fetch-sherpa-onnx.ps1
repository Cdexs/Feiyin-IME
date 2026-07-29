# fetch-sherpa-onnx.ps1
#
# 一键获取 Windows 版 sherpa-onnx 预编译共享库。
#
# 背景：vendor/sherpa-onnx/ 下的原生库文件被 .gitignore 排除（.gitignore:22 /vendor/sherpa-onnx/*-Release/），
# 因此任何平台的全新 git checkout 都无法直接构建。本脚本供 Windows 新开发者在 clone 后运行，
# 自动下载并解压对应版本的预编译包，使 SHERPA_ONNX_LIB_DIR（.cargo/config.toml）指向的路径存在。
#
# 用法：
#   .\scripts\fetch-sherpa-onnx.ps1 [-Version "1.12.38"] [-OutDir "vendor\sherpa-onnx"]
#
# 幂等：如果目标目录已存在，脚本会直接跳过。

param(
    [string]$Version = "1.12.38",
    [string]$OutDir = "vendor\sherpa-onnx",
    [string]$Flavor = "win-x64-shared-MD-Release"
)

$ErrorActionPreference = "Stop"

$packageName = "sherpa-onnx-v$Version-$Flavor"
$archiveName = "$packageName.tar.bz2"
$url = "https://github.com/k2-fsa/sherpa-onnx/releases/download/v$Version/$archiveName"
$outFile = Join-Path $OutDir $archiveName
$expectedDir = Join-Path $OutDir $packageName

# 幂等：目标目录已存在则跳过
if (Test-Path $expectedDir) {
    Write-Host "sherpa-onnx 已存在：$expectedDir"
    exit 0
}

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

Write-Host "正在下载 sherpa-onnx v$Version ($Flavor) ..."
Invoke-WebRequest -Uri $url -OutFile $outFile

Write-Host "正在解压 $archiveName ..."
# Windows 10/11 自带 tar 支持 bzip2
tar -xjf $outFile -C $OutDir

Write-Host "sherpa-onnx 已解压到 $expectedDir"

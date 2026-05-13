@echo off
REM voice-ime 构建脚本
REM 使用方法：双击运行或在 cmd 中执行

cd /d D:\Workspace\CodeLab\voice-ime

REM 设置 sherpa-onnx 库路径（使用 vendor 目录预下载的库）
set SHERPA_ONNX_LIB_DIR=D:\Workspace\CodeLab\voice-ime\vendor\sherpa-onnx\sherpa-onnx-v1.12.38-win-x64-shared-MD-Release\lib

REM 执行构建
cargo build --release

echo.
if %ERRORLEVEL% EQU 0 (
    echo 构建成功！
    echo 输出文件：target\release\voice-ime.exe

    REM 复制 DLL 到 release 目录
    copy /Y vendor\sherpa-onnx\sherpa-onnx-v1.12.38-win-x64-shared-MD-Release\bin\*.dll target\release\

    REM 同步 EXE 到 Publish/ 目录（发布暂存区）
    if exist Publish\ (
        echo 正在同步 EXE 到 Publish/ ...
        copy /Y target\release\voice-ime.exe Publish\
        if exist target\release\voice-ime-ui.exe copy /Y target\release\voice-ime-ui.exe Publish\
        if exist target\release\crash-reporter.exe copy /Y target\release\crash-reporter.exe Publish\
        echo EXE 已同步到 Publish/
    )
) else (
    echo 构建失败！
)
pause
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
    echo 输出文件：target\release\feiyin-ime.exe

    REM 复制 DLL 到 release 目录
    copy /Y vendor\sherpa-onnx\sherpa-onnx-v1.12.38-win-x64-shared-MD-Release\bin\*.dll target\release\

    REM 同步所有构建产物到 Publish/ 目录（发布暂存区，不可跳过）
    echo 正在同步产出物到 Publish/ ...
    if not exist Publish\ mkdir Publish\
    REM 清理旧 exe 名（重命名后遗留）
    del /F /Q Publish\voice-ime.exe 2>nul
    del /F /Q Publish\voice-ime-ui.exe 2>nul
    REM 同步 EXE
    copy /Y target\release\feiyin-ime.exe Publish\
    copy /Y target\release\feiyin-ime-ui.exe Publish\
    copy /Y target\release\crash-reporter.exe Publish\
    REM 同步运行时 DLL
    copy /Y target\release\sherpa-onnx-c-api.dll Publish\ 2>nul
    copy /Y target\release\sherpa-onnx-cxx-api.dll Publish\ 2>nul
    copy /Y target\release\onnxruntime.dll Publish\ 2>nul
    copy /Y target\release\onnxruntime_providers_shared.dll Publish\ 2>nul
    copy /Y target\release\ctranslate2.dll Publish\ 2>nul
    copy /Y target\release\libiomp5md.dll Publish\ 2>nul
    if exist target\release\cudnn64_9.dll copy /Y target\release\cudnn64_9.dll Publish\ 2>nul
    echo 产出物已同步到 Publish/
    dir Publish\*.exe Publish\*.dll
) else (
    echo 构建失败！
)
pause

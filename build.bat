@echo off
REM voice-ime 构建脚本
REM 使用方法：双击运行或在 cmd 中执行

cd /d D:\Workspace\CodeLab\voice-ime

REM FLASH-P3-A：输入法不再带本地模型，不再需要 SHERPA_ONNX_LIB_DIR 与模型运行库 DLL。

REM 执行构建
cargo build --release

echo.
if %ERRORLEVEL% EQU 0 (
    echo 构建成功！
    echo 输出文件：target\release\feiyin-ime.exe

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
    echo 产出物已同步到 Publish/
    dir Publish\*.exe Publish\*.dll
) else (
    echo 构建失败！
)
pause

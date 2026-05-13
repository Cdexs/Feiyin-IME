@echo off
cd /d D:\Workspace\CodeLab\voice-ime

echo === Checking for pytest ===
C:\Python314\python.exe -m pytest --version 2>nul
if %ERRORLEVEL% EQU 0 (
    echo === Running pytest ===
    C:\Python314\python.exe -m pytest tests/test_cases/ -m "not gui and not hardware" --tb=short -v 2>&1 | tee pytest_output.txt
) else (
    echo === Installing pytest ===
    C:\Python314\python.exe -m pip install --user --no-warn-script-location pytest pytest-timeout 2>&1
    echo === Running pytest ===
    C:\Python314\python.exe -m pytest tests/test_cases/ -m "not gui and not hardware" --tb=short -v 2>&1 | tee pytest_output.txt
)

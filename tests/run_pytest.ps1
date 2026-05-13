# run_pytest.ps1 - Run pytest for voice-ime tests
# Check if pytest is available
$pytestCmd = Get-Command pytest -ErrorAction SilentlyContinue
if (-not $pytestCmd) {
    Write-Host "pytest not found in PATH. Trying to install..."
    pip install pytest pytest-timeout pyautogui pywinauto psutil python-dotenv 2>&1
    if ($LASTEXITCODE -ne 0) {
        Write-Host "pip not available either. Checking for python..."
        $pythonCmd = Get-Command python -ErrorAction SilentlyContinue
        if (-not $pythonCmd) {
            $pythonCmd = Get-Command py -ErrorAction SilentlyContinue
        }
        if ($pythonCmd) {
            & $pythonCmd.Source -m pip install pytest pytest-timeout pyautogui pywinauto psutil python-dotenv 2>&1
        }
    }
    $pytestCmd = Get-Command pytest -ErrorAction SilentlyContinue
    if (-not $pytestCmd) {
        Write-Error "Cannot find or install pytest. Aborting."
        exit 1
    }
}

Write-Host "=== Tauri v2 Pytest Regression ==="
Write-Host "Working Directory: $(Get-Location)"
Write-Host "pytest location: $($pytestCmd.Source)"
Write-Host ""

cd D:\Workspace\CodeLab\voice-ime

pytest tests/test_cases/ -m "not gui and not hardware" --tb=short -v 2>&1 | Tee-Object -Variable pytestOutput
$exitCode = $LASTEXITCODE

Write-Host ""
Write-Host "=== Exit Code: $exitCode ==="

exit $exitCode

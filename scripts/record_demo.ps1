Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Pause-Step {
    param([string]$Message = "Press Enter to continue...")
    Write-Host ""
    Write-Host $Message -ForegroundColor Yellow
    [void][Console]::ReadLine()
}

function Run-Step {
    param(
        [string]$Title,
        [string]$DisplayCommand,
        [string]$LinuxCommand
    )

    Clear-Host
    Write-Host "============================================================" -ForegroundColor Cyan
    Write-Host $Title -ForegroundColor Cyan
    Write-Host "============================================================" -ForegroundColor Cyan
    Write-Host ""
    Write-Host "Command:" -ForegroundColor Green
    Write-Host $DisplayCommand -ForegroundColor White
    Write-Host ""
    Pause-Step "Press Enter to run this step..."
    & wsl -d Ubuntu-24.04 -- sh -lc $LinuxCommand
    Pause-Step "Step finished. Press Enter for the next step..."
}

Clear-Host
Write-Host "Project demo recording helper" -ForegroundColor Cyan
Write-Host ""
Write-Host "Run this inside the VS Code terminal, then record the VS Code window." -ForegroundColor White
Write-Host "This keeps the code and terminal output in one window for Xbox Game Bar." -ForegroundColor White
Pause-Step

Run-Step `
    -Title "Run the complete fail-closed final demo" `
    -DisplayCommand 'wsl -d Ubuntu-24.04 -- bash -lc "cd /mnt/d/project; bash scripts/final_demo.sh"' `
    -LinuxCommand 'cd /mnt/d/project; bash scripts/final_demo.sh'

Clear-Host
Write-Host "Demo finished" -ForegroundColor Cyan
Write-Host ""
Write-Host "Key points:" -ForegroundColor Green
Write-Host "1. The final summary must say FINAL: PASS."
Write-Host "2. Userspace correctness, Linux XTS compatibility, KVM evidence, and package checks are all covered."
Write-Host "3. Use scripts/final_demo.sh --live-kvm when a live KVM rerun is required."
Write-Host ""
Write-Host "You can stop recording now." -ForegroundColor Yellow

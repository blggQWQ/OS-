Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

Write-Host "dm-verity demo flow:"
Write-Host "1. Prepare a read-only data image."
Write-Host "2. Build DmVerityDevice::build(device) and record root_hash()."
Write-Host "3. Reopen with DmVerityDevice::open(device, tree, root_hash)."
Write-Host "4. Read normal blocks and confirm verification passes."
Write-Host "5. Tamper with the lower image and confirm reads fail with IntegrityViolation."


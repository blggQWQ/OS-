Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

Write-Host "dm-crypt demo flow:"
Write-Host "1. Implement BlockDevice for the Asterinas virtio-blk disk."
Write-Host "2. Wrap it with DmCryptDevice::new(device, key)."
Write-Host "3. Write plaintext through the wrapper."
Write-Host "4. Inspect the lower disk/image to confirm ciphertext-at-rest."
Write-Host "5. Read back through the wrapper to recover plaintext."


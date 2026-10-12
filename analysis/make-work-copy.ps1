# Creates an isolated working copy of the project for one subagent/task.
# Usage: pwsh analysis\make-work-copy.ps1 -Destination D:\...\analysis\work\<name>
param([Parameter(Mandatory = $true)][string]$Destination)

$src = "D:\code\Rasterbator"
$dst = (New-Item -ItemType Directory -Force -Path $Destination).FullName

foreach ($item in @("Cargo.toml", "Cargo.lock", ".gitignore")) {
    Copy-Item (Join-Path $src $item) $dst -Force
}
Copy-Item "$src\crates" "$dst\crates" -Recurse -Force
if (Test-Path "$src\analysis\refgen\images") {
    Copy-Item "$src\analysis\refgen\images" "$dst\test-images" -Recurse -Force
}
Write-Output "work copy created: $dst"
Write-Output "build: cargo test -p rasterbator-core   (run inside the copy)"

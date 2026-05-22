# Sync newtoki-scraper from ../my-tauri-app, then apply slim newtoki patches.
# Usage: powershell -ExecutionPolicy Bypass -File .\scripts\sync-from-my-tauri-app.ps1

$ErrorActionPreference = "Stop"
$Dst = Split-Path $PSScriptRoot -Parent
$Root = Split-Path $Dst -Parent
$Src = Join-Path $Root "my-tauri-app"
$Patch = Join-Path $PSScriptRoot "patches"

if (-not (Test-Path $Src)) {
    throw "Source project not found: $Src"
}

Write-Host "Source: $Src"
Write-Host "Target: $Dst"

function Copy-File($Rel) {
    $from = Join-Path $Src $Rel
    $to = Join-Path $Dst $Rel
    if (-not (Test-Path $from)) { throw "Missing: $from" }
    $dir = Split-Path $to -Parent
    if (-not (Test-Path $dir)) { New-Item -ItemType Directory -Path $dir -Force | Out-Null }
    Copy-Item -Path $from -Destination $to -Force
    Write-Host "  copied $Rel"
}

function Copy-Dir($Rel) {
    $from = Join-Path $Src $Rel
    $to = Join-Path $Dst $Rel
    if (-not (Test-Path $from)) { throw "Missing: $from" }
    if (Test-Path $to) { Remove-Item -Path $to -Recurse -Force }
    Copy-Item -Path $from -Destination $to -Recurse -Force
    Write-Host "  copied dir $Rel"
}

function Apply-Patch($Name, $DestRel) {
    Copy-Item -Path (Join-Path $Patch $Name) -Destination (Join-Path $Dst $DestRel) -Force
    Write-Host "  patched $DestRel"
}

function Remove-IfExists($Rel) {
    $path = Join-Path $Dst $Rel
    if (Test-Path $path) {
        Remove-Item -Path $path -Force
        Write-Host "  removed $Rel"
    }
}

Write-Host "`n[1/4] Copy shared modules from my-tauri-app..."
Copy-Dir  "src-tauri/src/core"
Copy-Dir  "src-tauri/src/scraper"
Copy-Dir  "src-tauri/src/templates"
Copy-File "src-tauri/templates/newtoki.json"
Copy-File "src/App.css"

Write-Host "`n[2/4] Copy optional docs..."
$notesSrc = Join-Path $Root "newtoki.notes.md"
$notesDst = Join-Path $Dst "docs/newtoki.notes.md"
if (Test-Path $notesSrc) {
    New-Item -ItemType Directory -Path (Split-Path $notesDst) -Force | Out-Null
    Copy-Item $notesSrc $notesDst -Force
    Write-Host "  copied newtoki.notes.md -> docs/"
}
$fixtureSrc = Join-Path $Root "newtoki-index"
$fixtureDst = Join-Path $Dst "docs/fixtures/newtoki-index"
if (Test-Path $fixtureSrc) {
    New-Item -ItemType Directory -Path (Split-Path $fixtureDst) -Force | Out-Null
    Copy-Item $fixtureSrc $fixtureDst -Force
    Write-Host "  copied newtoki-index -> docs/fixtures/"
}

Write-Host "`n[3/4] Apply newtoki slim patches..."
Apply-Patch "lib.rs"           "src-tauri/src/lib.rs"
Apply-Patch "newtoki.json"       "src-tauri/templates/newtoki.json"
Apply-Patch "built_in.rs"        "src-tauri/src/templates/built_in.rs"
Apply-Patch "core-mod.rs"        "src-tauri/src/core/mod.rs"
Apply-Patch "error.rs"           "src-tauri/src/core/error.rs"
Apply-Patch "scraper-mod.rs"     "src-tauri/src/scraper/mod.rs"
Apply-Patch "engine-mod.rs"      "src-tauri/src/scraper/engine/mod.rs"
Apply-Patch "source.rs"          "src-tauri/src/scraper/engine/source.rs"
Apply-Patch "novel.rs"           "src-tauri/src/scraper/engine/novel.rs"
Apply-Patch "commands.rs"        "src-tauri/src/commands.rs"
Apply-Patch "webview_fetch.rs"   "src-tauri/src/webview_fetch.rs"
Apply-Patch "templates-mod.rs"   "src-tauri/src/templates/mod.rs"
Apply-Patch "Cargo.toml"         "src-tauri/Cargo.toml"
Apply-Patch "fetcher.json"       "src-tauri/capabilities/fetcher.json"
Apply-Patch "default-capability.json" "src-tauri/capabilities/default.json"

Remove-IfExists "src-tauri/src/core/storage.rs"
Remove-IfExists "src-tauri/src/scraper/extractor.rs"
Remove-IfExists "src-tauri/src/scraper/http.rs"
Remove-IfExists "src-tauri/src/scraper/nextdata.rs"
Remove-IfExists "src-tauri/src/scraper/selector.rs"
Remove-IfExists "src-tauri/src/templates/registry.rs"
Remove-IfExists "src-tauri/src/templates/user.rs"

Write-Host "`n[4/4] Apply release UI..."
Apply-Patch "App.tsx" "src/App.tsx"

$cssPath = Join-Path $Dst "src/App.css"
$css = [System.IO.File]::ReadAllText($cssPath)
if ($css -notmatch '\.panel \{') {
    $extra = @'

.panel {
  max-width: 640px;
  margin: 0 auto;
  text-align: left;
}

.field {
  margin-bottom: 1rem;
}

.field label {
  display: block;
  margin-bottom: 0.35rem;
  font-weight: 500;
}

.field input {
  width: 100%;
  padding: 0.5rem 0.65rem;
}

.row {
  display: flex;
  gap: 0.5rem;
}

.row input {
  flex: 1;
}

button.primary {
  width: 100%;
  padding: 0.75rem;
  font-size: 1rem;
}

button.primary:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.hint {
  margin-top: 0.75rem;
  font-size: 0.85rem;
  color: #666;
}

.log {
  margin-top: 1.5rem;
  padding: 0.75rem;
  background: #f5f5f5;
  border-radius: 6px;
  max-height: 320px;
  overflow-y: auto;
  font-family: ui-monospace, monospace;
  font-size: 0.8rem;
}

.log .done {
  margin-top: 0.5rem;
  color: #0a7;
  font-weight: 600;
}
'@
    [System.IO.File]::WriteAllText($cssPath, $css + $extra)
    Write-Host "  appended panel styles to App.css"
}

Write-Host "`nDone. Run: npm run tauri dev"

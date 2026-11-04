# A plugin's [[build]] step on Windows: puts its binary in target\release\, as
# cargo would. The binary comes from the release tagged on the very commit
# Herdr checked out, checked against that release's SHA256SUMS; the checkout
# must be unchanged. Without such a release (a push to main still being
# published, a commit never released, a fork), it builds with cargo when cargo
# is there. Run from the plugin's folder with its binary's name:
# powershell -NoProfile -ExecutionPolicy Bypass -File ..\scripts\fetch-bin.ps1 herdr-menu
# Windows PowerShell 5.1, which every Windows has. See
# docs/en/adr/0008-prebuilt-binaries.md.
param([Parameter(Mandatory = $true)][string]$Name)
$ErrorActionPreference = 'Stop'

function Fail($message) {
    [Console]::Error.WriteLine("fetch-bin: $message")
    exit 1
}

# No release binary to use: builds with cargo, or says why it cannot.
function Build($why) {
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        Fail "$why; no cargo to build it with either (Rust 1.95 or later, and the Visual Studio Build Tools)"
    }
    [Console]::Error.WriteLine("fetch-bin: ${why}: building with cargo")
    & cargo build --release --locked --target-dir target
    exit $LASTEXITCODE
}

# A native command, its output as one string; $null when it fails. Windows
# PowerShell 5.1 turns a native command's stderr into an error, which 'Stop'
# would make fatal.
function Run($program) {
    $ErrorActionPreference = 'Continue'
    $out = & $program @args 2>$null
    if ($LASTEXITCODE) { return $null }
    return ($out -join "`n")
}

# Windows on ARM64 runs the x86_64 binary in emulation. A 32-bit PowerShell on
# a 64-bit Windows gives its own architecture in PROCESSOR_ARCHITECTURE, the
# machine's in PROCESSOR_ARCHITEW6432.
$machine = if ($env:PROCESSOR_ARCHITEW6432) { $env:PROCESSOR_ARCHITEW6432 } else { $env:PROCESSOR_ARCHITECTURE }
if ($machine -ne 'AMD64' -and $machine -ne 'ARM64') { Build "no release binary for Windows $machine" }
$asset = "$Name-x86_64-pc-windows-msvc.exe"

if (-not (Get-Command git -ErrorAction SilentlyContinue)) { Build 'git not found' }
$commit = Run git rev-parse HEAD
if (-not $commit) { Build 'not a Git checkout' }
$changes = Run git status --porcelain
if ($null -eq $changes -or $changes) { Build "files changed since commit $commit" }

$origin = Run git config --get remote.origin.url
$repo = $null
if ("$origin" -cmatch '^(?:https://|ssh://git@|git@)github\.com[:/]([A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+?)(?:\.git)?$') {
    $repo = $Matches[1]
}
if (-not $repo) { Build 'origin is not a GitHub repository' }

# The tag on the commit: a lightweight one gives the commit itself, an
# annotated one gives it on its peeled (^{}) line.
$tag = $null
foreach ($line in ("$(Run git ls-remote --tags origin)" -split "`n")) {
    $parts = $line -split '\s+'
    if ($parts.Count -ge 2 -and $parts[0] -ceq $commit -and $parts[1] -cmatch '^refs/tags/(v[^\^]+)(\^\{\})?$') {
        $tag = $Matches[1]
        break
    }
}
if (-not $tag) { Build "commit $commit has no release yet (one just pushed is published within minutes: herdr plugin update then)" }
$url = "https://github.com/$repo/releases/download/$tag"

# curl.exe, not curl: Windows PowerShell 5.1 has curl as an alias of
# Invoke-WebRequest. Windows 10 1803 and later have it.
if (-not (Get-Command curl.exe -ErrorAction SilentlyContinue)) { Build 'curl.exe not found' }

$dir = Join-Path (Get-Location) 'target\release'
New-Item -ItemType Directory -Force -Path $dir | Out-Null
$tmp = Join-Path $dir ".fetch-$PID"
try {
    if ($null -eq (Run curl.exe -fsSL -o $tmp "$url/$asset")) { Build "cannot download $url/$asset" }
    if ($null -eq (Run curl.exe -fsSL -o "$tmp.sums" "$url/SHA256SUMS")) { Build "cannot download $url/SHA256SUMS" }
    $want = $null
    foreach ($line in [IO.File]::ReadAllLines("$tmp.sums")) {
        $parts = $line -split '\s+'
        if ($parts.Count -ge 2 -and ($parts[1] -ceq $asset -or $parts[1] -ceq "*$asset")) { $want = $parts[0]; break }
    }
    if ($want -cnotmatch '^[0-9a-f]{64}$') { Fail "$asset is not in the SHA256SUMS of $tag" }
    # .NET, not Get-FileHash: Windows PowerShell 5.1 loses it when it inherits
    # PowerShell 7's PSModulePath, as when Herdr was started from PowerShell 7.
    $sha = [Security.Cryptography.SHA256]::Create()
    $stream = [IO.File]::OpenRead($tmp)
    try {
        $got = [BitConverter]::ToString($sha.ComputeHash($stream)).Replace('-', '').ToLowerInvariant()
    } finally {
        $stream.Dispose()
        $sha.Dispose()
    }
    if ($got -ne $want) { Fail "$asset does not match the SHA256SUMS of ${tag}: not installed" }

    Move-Item -LiteralPath $tmp -Destination (Join-Path $dir "$Name.exe") -Force
    Write-Host "fetch-bin: $Name $tag ($asset)"
} finally {
    Remove-Item -LiteralPath $tmp, "$tmp.sums" -Force -ErrorAction SilentlyContinue
}

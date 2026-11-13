# Owlmic Windows Setup Builder
# Builds owlmic.exe with static CRT, downloads drivers, and compiles Owlmic-Setup-x.y.z.exe via Inno Setup.

param (
    [switch]$SkipBuild = $false,
    [switch]$NoPause = $false
)

$ErrorActionPreference = "Stop"

$InstallerDir = $PSScriptRoot
$PcDir = Split-Path -Parent (Split-Path -Parent $InstallerDir)
$RepoDir = Split-Path -Parent $PcDir

function Write-Step([string]$message) {
    Write-Host "`n==> $message" -ForegroundColor Cyan
}

function Write-Success([string]$message) {
    Write-Host "[OK] $message" -ForegroundColor Green
}

function Write-Warn([string]$message) {
    Write-Host "[WARN] $message" -ForegroundColor Yellow
}

function Write-Err([string]$message) {
    Write-Host "[ERROR] $message" -ForegroundColor Red
}

# 1. Check / Download softcam.dll
Write-Step "Checking softcam.dll..."
$softcamPath = Join-Path $PcDir "softcam.dll"
$expectedSoftcamHash = '9D635E0AF682A883C3C7D407513D47E59B0883771701B101216820DFCF997F0B'
$needSoftcam = $true
if (Test-Path $softcamPath) {
    $existingHash = (Get-FileHash $softcamPath -Algorithm SHA256).Hash
    if ($existingHash -eq $expectedSoftcamHash) {
        $needSoftcam = $false
        Write-Success "softcam.dll is present and verified."
    }
}

if ($needSoftcam) {
    Write-Host "Downloading softcam.dll..."
    $url = 'https://raw.githubusercontent.com/diveshpatil9104/owlmic/3f78b5767f64f4542b4307c586da4b609d54d38e/pc/softcam.dll'
    Invoke-WebRequest -Uri $url -OutFile $softcamPath -UseBasicParsing
    $hash = (Get-FileHash $softcamPath -Algorithm SHA256).Hash
    if ($hash -ne $expectedSoftcamHash) {
        throw "Downloaded softcam.dll checksum mismatch: $hash"
    }
    Write-Success "Downloaded and verified softcam.dll."
}

# 2. Check / Download microphone driver pack
Write-Step "Checking microphone driver pack..."
$driverDir = Join-Path $InstallerDir "driver"
$driverSetupFile = Join-Path $driverDir "VBCABLE_Setup_x64.exe"

if (-not (Test-Path $driverSetupFile)) {
    Write-Host "Downloading virtual audio cable driver pack..."
    $tempZip = Join-Path $env:TEMP "owlmic-mic-driver.zip"
    $expectedDriverHash = 'B950E39F01AF1D04EA623C8F6D8EB9B6EA5C477C637295FABF20631C85116BFB'
    $sources = @(
        'https://download.vb-audio.com/Download_CABLE/VBCABLE_Driver_Pack45.zip',
        'https://web.archive.org/web/20240901000000id_/https://download.vb-audio.com/Download_CABLE/VBCABLE_Driver_Pack45.zip'
    )
    $downloaded = $false
    foreach ($url in $sources) {
        try {
            Write-Host "Trying $url..."
            if (Test-Path $tempZip) { Remove-Item $tempZip -Force }
            Invoke-WebRequest -Uri $url -OutFile $tempZip -UseBasicParsing -UserAgent 'Mozilla/5.0 (Windows NT 10.0; Win64; x64)' -TimeoutSec 60
            $hash = (Get-FileHash $tempZip -Algorithm SHA256).Hash
            if ($hash -eq $expectedDriverHash) {
                $downloaded = $true
                Write-Success "Driver package downloaded and verified."
                break
            }
            Write-Warn "Checksum mismatch for ${url}: $hash"
        } catch {
            Write-Warn "Download failed for ${url}: $($_.Exception.Message)"
        }
    }

    if (-not $downloaded) {
        throw "Failed to download verified driver pack from sources."
    }

    Write-Host "Extracting driver files to $driverDir..."
    if (-not (Test-Path $driverDir)) { New-Item -ItemType Directory -Path $driverDir | Out-Null }
    Expand-Archive -Path $tempZip -DestinationPath $driverDir -Force
    Remove-Item $tempZip -Force -ErrorAction SilentlyContinue
    Write-Success "Driver files unpacked."
} else {
    Write-Success "Microphone driver files are already present in pc\installer\driver."
}

# 3. Build release binaries
if (-not $SkipBuild) {
    Write-Step "Building owlmic.exe and owlmic_vcam.dll (Release)..."
    Push-Location $PcDir
    try {
        # Try static CRT first if supported by the toolchain
        $built = $false
        try {
            $env:RUSTFLAGS = "-C target-feature=+crt-static"
            cargo build --release
            if ($LASTEXITCODE -eq 0) { $built = $true }
        } catch {
            Write-Warn "Static CRT build failed, falling back to default release build..."
        }

        if (-not $built) {
            $env:RUSTFLAGS = ""
            cargo build --release
            if ($LASTEXITCODE -ne 0) {
                if ((Test-Path (Join-Path $PcDir "target\release\owlmic.exe")) -and (Test-Path (Join-Path $PcDir "target\release\owlmic_vcam.dll"))) {
                    Write-Warn "cargo build encountered an error, but target\release binaries already exist. Continuing with existing binaries."
                } else {
                    throw "cargo build --release failed with exit code $LASTEXITCODE"
                }
            }
        }
    } finally {
        $env:RUSTFLAGS = ""
        Pop-Location
    }
    Write-Success "Build completed."
} else {
    Write-Host "Skipping cargo build (-SkipBuild specified)."
}

# Verify required build outputs exist
$exePath = Join-Path $PcDir "target\release\owlmic.exe"
$vcamPath = Join-Path $PcDir "target\release\owlmic_vcam.dll"
if (-not (Test-Path $exePath)) { throw "Missing expected binary: $exePath" }
if (-not (Test-Path $vcamPath)) { throw "Missing expected library: $vcamPath" }

# 4. Find Inno Setup compiler (ISCC.exe)
Write-Step "Locating Inno Setup compiler (ISCC.exe)..."
$isccCmd = Get-Command "ISCC.exe" -ErrorAction SilentlyContinue
$isccPath = $null

if ($isccCmd) {
    $isccPath = $isccCmd.Source
} else {
    $possiblePaths = @(
        "${env:ProgramFiles(x86)}\Inno Setup *\ISCC.exe",
        "$env:ProgramFiles\Inno Setup *\ISCC.exe",
        "${env:LOCALAPPDATA}\Programs\Inno Setup *\ISCC.exe"
    )
    $found = Get-ChildItem -Path $possiblePaths -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($found) {
        $isccPath = $found.FullName
    }
}

if (-not $isccPath) {
    Write-Warn "Inno Setup compiler (ISCC.exe) was not found."
    Write-Host "Attempting to install Inno Setup via winget..."
    try {
        winget install --id JRSoftware.InnoSetup -e --accept-source-agreements --accept-package-agreements --silent
        # Refresh search
        $possiblePaths = @(
            "${env:ProgramFiles(x86)}\Inno Setup *\ISCC.exe",
            "$env:ProgramFiles\Inno Setup *\ISCC.exe",
            "${env:LOCALAPPDATA}\Programs\Inno Setup *\ISCC.exe"
        )
        $found = Get-ChildItem -Path $possiblePaths -ErrorAction SilentlyContinue | Select-Object -First 1
        if ($found) {
            $isccPath = $found.FullName
        }
    } catch {
        Write-Warn "Automatic installation via winget failed or requires restart."
    }
}

if (-not $isccPath) {
    throw "Inno Setup is required to build the installer. Please install Inno Setup 6 (https://jrsoftware.org/isdl.php) and re-run this script."
}

Write-Success "Found Inno Setup at: $isccPath"

# 5. Get Version from pc/Cargo.toml
$cargoTomlPath = Join-Path $PcDir "Cargo.toml"
$cargoToml = Get-Content $cargoTomlPath -Raw
if ($cargoToml -match '(?m)^version\s*=\s*"([^"]+)"') {
    $version = $Matches[1]
} else {
    $version = "0.1.0"
}
Write-Host "Building setup for Owlmic version $version..."

# 6. Compile Inno Setup Script
Write-Step "Compiling Owlmic installer..."
$issPath = Join-Path $InstallerDir "owlmic.iss"
$outputDir = Join-Path $InstallerDir "Output"

if (-not (Test-Path $outputDir)) {
    New-Item -ItemType Directory -Path $outputDir | Out-Null
}

& "$isccPath" "/DMyAppVersion=$version" "$issPath"
if ($LASTEXITCODE -ne 0) {
    throw "Inno Setup compilation failed with exit code $LASTEXITCODE"
}

# Copy standalone binary to output as well (matching CI behavior)
$standaloneCopy = Join-Path $outputDir "Owlmic-v$version-windows-x64.exe"
Copy-Item $exePath $standaloneCopy -Force

$setupExe = Join-Path $outputDir "Owlmic-Setup-$version.exe"
if (Test-Path $setupExe) {
    $setupSizeMB = [math]::Round(((Get-Item $setupExe).Length / 1MB), 2)
    Write-Host ""
    Write-Host "==========================================================" -ForegroundColor Green
    Write-Success "SETUP FILE CREATED SUCCESSFULLY!"
    Write-Host "Installer Location: $setupExe" -ForegroundColor Cyan
    Write-Host "Installer Size:     $setupSizeMB MB" -ForegroundColor Cyan
    Write-Host "Standalone Binary:  $standaloneCopy" -ForegroundColor Cyan
    Write-Host "==========================================================" -ForegroundColor Green
} else {
    throw "Setup executable was not found in $outputDir"
}

if (-not $NoPause) {
    Write-Host "`nPress Enter to exit..."
    Read-Host | Out-Null
}

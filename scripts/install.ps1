<#
.SYNOPSIS
    Vidown automated installer and uninstaller for Windows.
.DESCRIPTION
    Downloads the latest release of Vidown from GitHub, extracts it to
    C:\Program Files\Vidown\bin, configures the Machine PATH environment
    variable, and enables immediate terminal usage.
.PARAMETER Uninstall
    Removes Vidown from C:\Program Files\Vidown and cleans the entry from Machine PATH.
#>
[CmdletBinding()]
param(
    [switch]$Uninstall
)

$ErrorActionPreference = "Stop"

$Repo = if ($env:VIDOWN_REPO) { $env:VIDOWN_REPO } else { "TheNobodyBaruah/Vidown" }
$InstallDir = if ($env:VIDOWN_INSTALL_DIR) { $env:VIDOWN_INSTALL_DIR } else { "C:\Program Files\Vidown" }
$BinDir = Join-Path $InstallDir "bin"

# 1. Verify Administrator privileges
$isAdmin = if ($env:VIDOWN_TEST_BYPASS_ADMIN -eq "1") { $true } else { ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator) }
if (-not $isAdmin) {
    Write-Error "Administrator privileges are required. Please re-run this script in PowerShell as Administrator ('Run as administrator')."
    exit 1
}

# 2. Check for running Vidown processes
$running = Get-Process -Name "vidown" -ErrorAction SilentlyContinue
if ($running) {
    if ($Uninstall) {
        Write-Warning "Vidown is currently running. Please close all running instances of Vidown before uninstalling."
    } else {
        Write-Warning "Vidown is currently running. Please close all running instances of Vidown before installing/updating."
    }
}

# 3. Handle Uninstallation
if ($Uninstall) {
    Write-Host "==> Uninstalling Vidown..." -ForegroundColor Cyan

    if (Test-Path $InstallDir) {
        try {
            Remove-Item -Path $InstallDir -Recurse -Force
            Write-Host "Removed $InstallDir" -ForegroundColor Green
        } catch {
            Write-Error "Failed to remove $InstallDir. Please ensure no running processes or open terminals are using files in this folder. Details: $_"
            exit 1
        }
    } else {
        Write-Host "Installation folder $InstallDir not found." -ForegroundColor Yellow
    }

    # Clean Machine PATH
    try {
        $machinePath = [System.Environment]::GetEnvironmentVariable("Path", "Machine")
        if ($machinePath) {
            $paths = $machinePath -split ';' | Where-Object { $_ -and $_.Trim() -and ($_.Trim().TrimEnd('\') -ine $BinDir.TrimEnd('\')) }
            $newMachinePath = ($paths -join ';')
            if ($newMachinePath -ne $machinePath) {
                [System.Environment]::SetEnvironmentVariable("Path", $newMachinePath, "Machine")
                Write-Host "Removed $BinDir from Machine PATH." -ForegroundColor Green
            }
        }
    } catch {
        Write-Warning "Could not update Machine PATH: $_"
    }

    # Clean current session PATH
    $sessionPaths = $env:Path -split ';' | Where-Object { $_ -and $_.Trim() -and ($_.Trim().TrimEnd('\') -ine $BinDir.TrimEnd('\')) }
    $env:Path = ($sessionPaths -join ';')

    Write-Host "==> Vidown uninstalled successfully." -ForegroundColor Green
    return
}

# 4. Installation Flow
Write-Host "==> Installing Vidown..." -ForegroundColor Cyan

# Ensure modern TLS protocols are enabled across .NET Framework versions
try {
    [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12 -bor [Net.SecurityProtocolType]::Tls13
} catch {
    [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
}

$ApiUrl = if ($env:VIDOWN_API_URL) { $env:VIDOWN_API_URL } else { "https://api.github.com/repos/$Repo/releases/latest" }
Write-Host "==> Fetching latest release information from GitHub ($ApiUrl)..." -ForegroundColor Cyan

try {
    $release = Invoke-RestMethod -Uri $ApiUrl -Headers @{ "User-Agent" = "Vidown-Installer" }
} catch {
    Write-Error "Failed to query GitHub Releases API at $ApiUrl. If no release has been published yet, please create a release at https://github.com/$Repo/releases. Details: $_"
    exit 1
}

$asset = $release.assets | Where-Object { $_.name -like "*windows-x86_64.zip" } | Select-Object -First 1
if (-not $asset) {
    Write-Error "Could not find a windows-x86_64.zip asset in the latest release. Please verify that a release exists at https://github.com/$Repo/releases."
    exit 1
}

$tempZip = Join-Path $env:TEMP ("vidown-install-" + [System.Guid]::NewGuid().ToString() + ".zip")
$tempExtract = Join-Path $env:TEMP ("vidown-extract-" + [System.Guid]::NewGuid().ToString())

try {
    Write-Host "==> Downloading $($asset.name)..." -ForegroundColor Cyan
    Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $tempZip -UseBasicParsing

    Write-Host "==> Extracting archive..." -ForegroundColor Cyan
    Expand-Archive -Path $tempZip -DestinationPath $tempExtract -Force

    $foundExe = Get-ChildItem -Path $tempExtract -Filter "vidown.exe" -Recurse | Select-Object -First 1
    if (-not $foundExe) {
        Write-Error "Could not find vidown.exe in extracted archive."
        exit 1
    }

    if (-not (Test-Path $BinDir)) {
        New-Item -ItemType Directory -Path $BinDir -Force | Out-Null
    }

    $targetExe = Join-Path $BinDir "vidown.exe"
    $targetExeCap = Join-Path $BinDir "Vidown.exe"

    Copy-Item -Path $foundExe.FullName -Destination $targetExe -Force
    Write-Host "Installed $targetExe" -ForegroundColor Green

    # Copy as Vidown.exe per requirements (if case-sensitive filesystem or separate alias)
    try {
        Copy-Item -Path $foundExe.FullName -Destination $targetExeCap -Force -ErrorAction SilentlyContinue
    } catch {
        # File system is case-insensitive; vidown.exe already matches both vidown and Vidown
    }

    # 5. Configure Machine PATH
    try {
        $machinePath = [System.Environment]::GetEnvironmentVariable("Path", "Machine")
        $paths = if ($machinePath) { $machinePath -split ';' | Where-Object { $_ -and $_.Trim() } } else { @() }
        $alreadyInMachinePath = $paths | Where-Object { $_.Trim().TrimEnd('\') -ieq $BinDir.TrimEnd('\') }

        if (-not $alreadyInMachinePath) {
            $newMachinePath = ($paths + $BinDir) -join ';'
            [System.Environment]::SetEnvironmentVariable("Path", $newMachinePath, "Machine")
            Write-Host "Added $BinDir to Machine PATH." -ForegroundColor Green
        } else {
            Write-Host "$BinDir is already in Machine PATH." -ForegroundColor Yellow
        }
    } catch {
        Write-Warning "Could not update Machine PATH: $_"
    }

    # 6. Update Current Session PATH
    $sessionPaths = $env:Path -split ';' | Where-Object { $_ -and $_.Trim() }
    $alreadyInSessionPath = $sessionPaths | Where-Object { $_.Trim().TrimEnd('\') -ieq $BinDir.TrimEnd('\') }
    if (-not $alreadyInSessionPath) {
        $env:Path = ($sessionPaths + $BinDir) -join ';'
    }

    Write-Host "`n==> Vidown installed successfully!" -ForegroundColor Green
    Write-Host "==> You can now run 'Vidown' or 'vidown' in your terminal." -ForegroundColor Green
} finally {
    if (Test-Path $tempZip) {
        Remove-Item -Path $tempZip -Force -ErrorAction SilentlyContinue
    }
    if (Test-Path $tempExtract) {
        Remove-Item -Path $tempExtract -Recurse -Force -ErrorAction SilentlyContinue
    }
}

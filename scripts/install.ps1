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

# Defense-in-depth check for VCRUNTIME140.dll
$systemRoot = if ($env:SystemRoot) { $env:SystemRoot } elseif ($env:windir) { $env:windir } else { "C:\Windows" }
$vcRuntimeDll = if ($env:VIDOWN_TEST_VCRUNTIME_DLL) { $env:VIDOWN_TEST_VCRUNTIME_DLL } else { "$systemRoot\System32\vcruntime140.dll" }
if (-not (Test-Path $vcRuntimeDll)) {
    Write-Host "==> Visual C++ runtime (vcruntime140.dll) not found. Installing Visual C++ 2015-2022 Redistributable..." -ForegroundColor Yellow
    $vcRedistUrl = if ($env:VIDOWN_VCREDIST_URL) { $env:VIDOWN_VCREDIST_URL } else { "https://aka.ms/vs/17/release/vc_redist.x64.exe" }
    $tempVcRedist = Join-Path $env:TEMP ("vc_redist.x64-" + [System.Guid]::NewGuid().ToString() + ".exe")
    try {
        Write-Host "==> Downloading Visual C++ 2015-2022 Redistributable ($vcRedistUrl)..." -ForegroundColor Cyan
        Invoke-WebRequest -Uri $vcRedistUrl -OutFile $tempVcRedist -UseBasicParsing

        Write-Host "==> Installing Visual C++ 2015-2022 Redistributable silently..." -ForegroundColor Cyan
        $vcProc = Start-Process -FilePath $tempVcRedist -ArgumentList "/quiet", "/norestart" -Wait -PassThru
        if ($vcProc) {
            $vcProc.WaitForExit()
            if ($vcProc.ExitCode -eq 0 -or $vcProc.ExitCode -eq 3010 -or $vcProc.ExitCode -eq 1638) {
                Write-Host "Visual C++ Redistributable installed successfully." -ForegroundColor Green
            } else {
                Write-Warning "Visual C++ Redistributable installer returned exit code $($vcProc.ExitCode)."
            }
        }
    } catch {
        Write-Warning "Failed to install Visual C++ Redistributable automatically: $_"
    } finally {
        if (Test-Path $tempVcRedist) {
            Remove-Item -Path $tempVcRedist -Force -ErrorAction SilentlyContinue
        }
    }
}

$downloadUrl = $null
$assetName = $null
$releaseTag = $null

$ApiUrl = if ($env:VIDOWN_API_URL) { $env:VIDOWN_API_URL } else { "https://api.github.com/repos/$Repo/releases/latest" }
Write-Host "==> Fetching latest release information from GitHub ($ApiUrl)..." -ForegroundColor Cyan

try {
    $release = Invoke-RestMethod -Uri $ApiUrl -Headers @{ "User-Agent" = "Vidown-Installer" }
    if ($release.tag_name) {
        $releaseTag = $release.tag_name
    }
    $asset = $release.assets | Where-Object { $_.name -like "*windows-x86_64.zip" } | Select-Object -First 1
    if ($asset) {
        $downloadUrl = $asset.browser_download_url
        $assetName = $asset.name
    }
} catch {
    Write-Warning "Could not query GitHub Releases API: $_"
}

if (-not $downloadUrl) {
    Write-Host "==> Falling back to direct latest release asset URL..." -ForegroundColor Yellow
    $downloadUrl = "https://github.com/$Repo/releases/latest/download/vidown-windows-x86_64.zip"
    $assetName = "vidown-windows-x86_64.zip"
    $releaseTag = "latest"
}

$tempZip = Join-Path $env:TEMP ("vidown-install-" + [System.Guid]::NewGuid().ToString() + ".zip")
$tempExtract = Join-Path $env:TEMP ("vidown-extract-" + [System.Guid]::NewGuid().ToString())

try {
    if ($releaseTag -and $releaseTag -ne "latest") {
        Write-Host "==> Downloading $assetName ($releaseTag)..." -ForegroundColor Cyan
    } else {
        Write-Host "==> Downloading $assetName from $downloadUrl..." -ForegroundColor Cyan
    }
    Invoke-WebRequest -Uri $downloadUrl -OutFile $tempZip -UseBasicParsing

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

    # Verify installed binary and display version
    try {
        $ver = & $targetExe --version 2>&1
        if ($ver) {
            Write-Host "==> Installed binary version: $ver" -ForegroundColor Green
        }
    } catch {
        # Non-critical if binary execution check fails
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

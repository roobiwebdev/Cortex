# Cortex installer for Windows.
#
#   irm https://cortex.roobi.dev/install.ps1 | iex
#
# Downloads the per-user NSIS installer from the latest GitHub release, runs it silently
# (no admin needed) and starts Cortex. The installer lands in %LOCALAPPDATA%\Cortex.
#
# A file that Invoke-WebRequest saves carries no Mark of the Web, so SmartScreen does not
# stop the installer or the installed app the way it does for a browser download.
#
# Environment overrides:
#   CORTEX_VERSION  release tag to install, e.g. v0.1.0 (default: the latest release)
#   INSTALL_DIR     install folder (default: %LOCALAPPDATA%\Cortex, or where Cortex already is)
#   NO_OPEN         set to 1 to skip starting Cortex after installing
#   CORTEX_API_URL  test only: release JSON URL to read instead of the GitHub API
#
# Runs in Windows PowerShell 5.1 and PowerShell 7. Everything happens inside a function so
# that `iex` leaves the caller's session settings alone; errors are thrown, never `exit`,
# so the user's window stays open. See docs/engineering/releasing.md.

function Install-Cortex {
    $ErrorActionPreference = 'Stop'
    $ProgressPreference = 'Continue'

    $repo = 'roobiwebdev/cortex-releases'
    $assetSuffix = '-Windows-x64-Setup.exe'
    $fallbackAsset = 'Cortex-Windows-x64-Setup.exe'
    # Tauri's NSIS template installs the cargo binary under its own name (MAINBINARYNAME).
    $processName = 'cortex-desktop'
    $exeName = 'cortex-desktop.exe'
    $headers = @{ 'User-Agent' = 'cortex-installer'; 'Accept' = 'application/vnd.github+json' }

    function Write-Step([string]$Message) { Write-Host "==> $Message" }

    $noOpen = $false
    if ($env:NO_OPEN -and @('0', 'false', 'no') -notcontains $env:NO_OPEN.ToLowerInvariant()) { $noOpen = $true }

    # Older Windows PowerShell defaults to TLS 1.0, which GitHub refuses.
    [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

    if (-not [Environment]::Is64BitOperatingSystem) {
        throw 'Cortex needs 64-bit Windows. This PC runs a 32-bit version of Windows.'
    }

    # The registry reports the real CPU even when PowerShell itself runs emulated.
    $machineArch = $null
    try {
        $machineArch = (Get-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager\Environment' -Name PROCESSOR_ARCHITECTURE).PROCESSOR_ARCHITECTURE
    } catch {
        $machineArch = $env:PROCESSOR_ARCHITECTURE
    }
    if ($machineArch -eq 'ARM64') {
        Write-Host 'This PC has an ARM processor. Cortex is an x64 app; Windows 11 runs it through its built-in x64 emulation.'
    }

    $tag = $null
    if ($env:CORTEX_VERSION) {
        $tag = $env:CORTEX_VERSION.Trim()
        if (-not $tag.StartsWith('v')) { $tag = "v$tag" }
    }

    if ($env:CORTEX_API_URL) {
        $apiUrl = $env:CORTEX_API_URL
    } elseif ($tag) {
        $apiUrl = "https://api.github.com/repos/$repo/releases/tags/$tag"
    } else {
        $apiUrl = "https://api.github.com/repos/$repo/releases/latest"
    }

    if ($tag) { Write-Step "Looking up Cortex $tag" } else { Write-Step 'Looking up the latest Cortex release' }
    $downloadUrl = $null
    try {
        $release = Invoke-RestMethod -Uri $apiUrl -Headers $headers -UseBasicParsing
        Write-Host "    Release: $($release.tag_name)"
        $asset = $release.assets | Where-Object { $_.name.EndsWith($assetSuffix) } | Select-Object -First 1
        if (-not $asset) {
            throw "Release $($release.tag_name) has no Windows installer. See https://github.com/$repo/releases"
        }
        $downloadUrl = $asset.browser_download_url
    } catch [System.Net.WebException] {
        $status = $null
        if ($_.Exception.Response) { $status = [int]$_.Exception.Response.StatusCode }
        if ($tag -and $status -eq 404 -and -not $env:CORTEX_API_URL) {
            throw "Cortex release $tag does not exist. See https://github.com/$repo/releases"
        }
        Write-Warning 'Could not read the release list from GitHub (it may be rate limiting this network). Trying the direct download link.'
    } catch {
        # PowerShell 7 raises HttpResponseException (HTTP errors) or HttpRequestException
        # (network errors) instead of WebException.
        $kind = $_.Exception.GetType().Name
        if ($kind -ne 'HttpResponseException' -and $kind -ne 'HttpRequestException') { throw }
        $status = $null
        if ($_.Exception.Response) { $status = [int]$_.Exception.Response.StatusCode }
        if ($tag -and $status -eq 404 -and -not $env:CORTEX_API_URL) {
            throw "Cortex release $tag does not exist. See https://github.com/$repo/releases"
        }
        Write-Warning 'Could not read the release list from GitHub (it may be rate limiting this network). Trying the direct download link.'
    }
    if (-not $downloadUrl) {
        if ($tag) {
            $downloadUrl = "https://github.com/$repo/releases/download/$tag/$fallbackAsset"
        } else {
            $downloadUrl = "https://github.com/$repo/releases/latest/download/$fallbackAsset"
        }
    }

    $setup = Join-Path ([IO.Path]::GetTempPath()) ("Cortex-Setup-{0}.exe" -f [Guid]::NewGuid().ToString('N'))
    try {
        Write-Step "Downloading $($downloadUrl.Substring($downloadUrl.LastIndexOf('/') + 1))"
        # The progress bar makes Invoke-WebRequest many times slower in Windows PowerShell 5.1.
        $ProgressPreference = 'SilentlyContinue'
        try {
            Invoke-WebRequest -Uri $downloadUrl -OutFile $setup -Headers @{ 'User-Agent' = 'cortex-installer' } -UseBasicParsing
        } catch {
            throw "Download failed: $downloadUrl ($($_.Exception.Message))"
        } finally {
            $ProgressPreference = 'Continue'
        }

        # Close a running Cortex so the installer can replace its files.
        $running = @(Get-Process -Name $processName -ErrorAction SilentlyContinue)
        if ($running.Count -gt 0) {
            Write-Step 'Closing the running Cortex'
            foreach ($p in $running) {
                try { [void]$p.CloseMainWindow() } catch { }
            }
            foreach ($p in $running) {
                try {
                    if (-not $p.WaitForExit(10000)) { Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue }
                } catch { }
            }
            Start-Sleep -Milliseconds 500
            if (@(Get-Process -Name $processName -ErrorAction SilentlyContinue).Count -gt 0) {
                throw 'Cortex is still running. Quit it (tray icon, then Quit) and run the installer again.'
            }
        }

        # /S is silent. /D= sets the folder; NSIS requires it last and unquoted, even with spaces.
        $arguments = '/S'
        if ($env:INSTALL_DIR) {
            $arguments = "/S /D=$($env:INSTALL_DIR.TrimEnd('\'))"
        }
        Write-Step 'Installing Cortex (per user, no admin needed)'
        $proc = Start-Process -FilePath $setup -ArgumentList $arguments -Wait -PassThru
        if ($proc.ExitCode -ne 0) {
            throw "The Cortex installer failed with exit code $($proc.ExitCode). Installing an older version over a newer one is refused; uninstall Cortex first in that case."
        }
    } finally {
        Remove-Item -LiteralPath $setup -Force -ErrorAction SilentlyContinue
    }

    # Where it went: INSTALL_DIR, else the folder the installer recorded, else the default.
    $installDir = $null
    if ($env:INSTALL_DIR) {
        $installDir = $env:INSTALL_DIR.TrimEnd('\')
    } else {
        try {
            $location = (Get-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Cortex' -Name InstallLocation).InstallLocation
            if ($location) { $installDir = $location.Trim('"') }
        } catch { }
        if (-not $installDir) { $installDir = Join-Path $env:LOCALAPPDATA 'Cortex' }
    }

    $exe = Join-Path $installDir $exeName
    if (-not (Test-Path -LiteralPath $exe)) {
        $found = Get-ChildItem -LiteralPath $installDir -Filter '*.exe' -ErrorAction SilentlyContinue |
            Where-Object { $_.Name -ne 'uninstall.exe' } | Select-Object -First 1
        if (-not $found) { throw "The installer finished, but Cortex was not found in $installDir." }
        $exe = $found.FullName
    }

    Write-Host ''
    Write-Host "Cortex is installed: $exe"
    Write-Host 'It is in the Start menu as Cortex.'
    if (-not $noOpen) {
        Write-Step 'Starting Cortex'
        Start-Process -FilePath $exe -WorkingDirectory $installDir
    }
}

Install-Cortex

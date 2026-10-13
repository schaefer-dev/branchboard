# Match the native driver to WebView2, rather than the separately installed Edge browser.
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$diagnostics = Join-Path $PWD 'e2e-results/smoke'
New-Item -ItemType Directory -Path $diagnostics -Force | Out-Null
$metadata = [ordered]@{
    runtimeVersion = $null
    runtimeRegistry = $null
    bundledDriverVersion = $null
    driverVersion = $null
    nativeDriver = $null
    error = $null
}

function Get-DriverVersion([string] $Path) {
    $output = & $Path --version
    if ($LASTEXITCODE -ne 0 -or "$output" -notmatch '\d+\.\d+\.\d+\.\d+') {
        throw "Could not read Edge WebDriver version from $Path ($output)"
    }
    return [version] $Matches[0]
}

try {
    $runtimeId = '{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}'
    $registryPaths = @(
        "HKCU:\Software\Microsoft\EdgeUpdate\Clients\$runtimeId",
        "HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\$runtimeId",
        "HKLM:\SOFTWARE\Microsoft\EdgeUpdate\Clients\$runtimeId"
    )
    $runtimeVersion = $null
    foreach ($path in $registryPaths) {
        $entry = Get-ItemProperty -Path $path -Name pv -ErrorAction SilentlyContinue
        if ($null -ne $entry -and $entry.pv -match '^\d+\.\d+\.\d+\.\d+$' -and ([version] $entry.pv).Major -gt 0) {
            $runtimeVersion = [version] $entry.pv
            $metadata.runtimeRegistry = $path
            break
        }
    }
    if ($null -eq $runtimeVersion) { throw 'No installed Evergreen WebView2 runtime was found in the registry.' }
    $metadata.runtimeVersion = $runtimeVersion.ToString()
    Write-Host "WebView2 runtime: $runtimeVersion ($($metadata.runtimeRegistry))"

    $driverPath = $null
    if ($env:EDGEWEBDRIVER) {
        $bundled = Join-Path $env:EDGEWEBDRIVER 'msedgedriver.exe'
        if (Test-Path $bundled -PathType Leaf) {
            $bundledVersion = Get-DriverVersion $bundled
            $metadata.bundledDriverVersion = $bundledVersion.ToString()
            Write-Host "Bundled Edge WebDriver: $bundledVersion"
            if ($bundledVersion.ToString(3) -eq $runtimeVersion.ToString(3)) { $driverPath = $bundled }
        }
    }

    if ($null -eq $driverPath) {
        $installDir = Join-Path $env:RUNNER_TEMP 'branchboard-edgedriver'
        New-Item -ItemType Directory -Path $installDir -Force | Out-Null
        $archive = Join-Path $installDir 'edgedriver.zip'
        $version = $runtimeVersion.ToString()
        $download = "https://msedgedriver.microsoft.com/$version/edgedriver_win64.zip"
        try {
            Invoke-WebRequest -Uri $download -OutFile $archive -TimeoutSec 60
        } catch {
            # WebView2 and Edge can have different patch versions of the same build.
            $release = Invoke-WebRequest -Uri "https://msedgedriver.microsoft.com/LATEST_RELEASE_$($runtimeVersion.Major)_WINDOWS" -TimeoutSec 30
            $releaseText = if ($release.Content -is [byte[]]) {
                [System.Text.Encoding]::Unicode.GetString($release.Content)
            } else { "$($release.Content)" }
            $releaseText = $releaseText -replace '\x00', ''
            if ($releaseText -notmatch '\d+\.\d+\.\d+\.\d+') { throw 'Microsoft did not return a valid Edge WebDriver release.' }
            $version = [version] $Matches[0]
            if ($version.ToString(3) -ne $runtimeVersion.ToString(3)) {
                throw "No compatible driver release found for WebView2 $runtimeVersion (available: $version)."
            }
            $download = "https://msedgedriver.microsoft.com/$version/edgedriver_win64.zip"
            Invoke-WebRequest -Uri $download -OutFile $archive -TimeoutSec 60
        }
        Write-Host "Downloaded matching driver: $download"
        Expand-Archive -Path $archive -DestinationPath $installDir -Force
        $driverPath = Join-Path $installDir 'msedgedriver.exe'
    }

    $driverVersion = Get-DriverVersion $driverPath
    if ($driverVersion.ToString(3) -ne $runtimeVersion.ToString(3)) {
        throw "Edge WebDriver $driverVersion is incompatible with WebView2 $runtimeVersion."
    }
    $driverPath = (Resolve-Path $driverPath).Path
    $metadata.driverVersion = $driverVersion.ToString()
    $metadata.nativeDriver = $driverPath
    Write-Host "Selected Edge WebDriver: $driverVersion ($driverPath)"
    "NATIVE_DRIVER=$driverPath" | Out-File -FilePath $env:GITHUB_ENV -Encoding utf8 -Append
} catch {
    $metadata.error = $_.Exception.Message
    throw
} finally {
    $metadata | ConvertTo-Json | Set-Content -Path (Join-Path $diagnostics 'webview2-setup.json') -Encoding utf8
}

# The runner is elevated; execute the actual install and app tests as a standard user.
param(
    [switch] $AsStandardUser,
    [string] $Stage,
    [string] $Node
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

function Invoke-BoundedProcess([string] $Program, [string[]] $Arguments, [int] $TimeoutSeconds = 120) {
    $process = Start-Process -FilePath $Program -ArgumentList $Arguments -NoNewWindow -PassThru
    $null = $process.Handle
    if (!$process.WaitForExit($TimeoutSeconds * 1000)) {
        Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
        throw "$Program did not finish within $TimeoutSeconds seconds"
    }
    if ($process.ExitCode -ne 0) { throw "$Program exited with code $($process.ExitCode)" }
}

function Get-Registration([string] $Hive) {
    $root = "${Hive}:\Software\Microsoft\Windows\CurrentVersion\Uninstall"
    return @(Get-ChildItem $root -ErrorAction SilentlyContinue | ForEach-Object {
        Get-ItemProperty $_.PSPath
    } | Where-Object { $_.PSObject.Properties['DisplayName'] -and $_.DisplayName -eq 'Branchboard' })
}

if ($AsStandardUser) {
    Start-Transcript -Path (Join-Path $Stage 'standard-user.log') | Out-Null
    try {
        $principal = [Security.Principal.WindowsPrincipal]::new([Security.Principal.WindowsIdentity]::GetCurrent())
        if ($principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
            throw 'Installer verification must run without administrator rights'
        }
        $profile = Get-ItemProperty "HKLM:\Software\Microsoft\Windows NT\CurrentVersion\ProfileList\$($principal.Identity.User.Value)"
        $env:USERPROFILE = [Environment]::ExpandEnvironmentVariables($profile.ProfileImagePath)
        $env:LOCALAPPDATA = Join-Path $env:USERPROFILE 'AppData/Local'
        $env:APPDATA = Join-Path $env:USERPROFILE 'AppData/Roaming'
        $env:TEMP = Join-Path $env:LOCALAPPDATA 'Temp'
        $env:TMP = $env:TEMP
        $env:RUNNER_TEMP = Join-Path $Stage 'temp'
        New-Item -ItemType Directory -Path $env:TEMP, $env:RUNNER_TEMP -Force | Out-Null
        $machineBefore = @(Get-Registration 'HKLM' | ForEach-Object { $_.PSPath })
        $install = Join-Path $env:LOCALAPPDATA 'Branchboard'
        $installer = Join-Path $Stage 'Branchboard-setup.exe'
        $app = Join-Path $install 'branchboard.exe'

        # /D must be the final argument, without quotes, including for paths with spaces.
        Invoke-BoundedProcess $installer @('/S', "/D=$install")
        if (!(Test-Path $app -PathType Leaf)) { throw "Installed application missing: $app" }
        $registration = @(Get-Registration 'HKCU')
        if ($registration.Count -ne 1) { throw 'Expected one per-user uninstall registration' }
        $machineAfter = @(Get-Registration 'HKLM' | ForEach-Object { $_.PSPath })
        if (($machineBefore -join "`n") -ne ($machineAfter -join "`n")) {
            throw 'Installation changed machine-wide Branchboard registration'
        }
        Invoke-BoundedProcess $installer @('/S', "/D=$install")

        $env:BRANCHBOARD_APP = $app
        $env:TAURI_DRIVER = Join-Path $Stage 'tauri-driver.exe'
        $env:NATIVE_DRIVER = Join-Path $Stage 'msedgedriver.exe'
        Push-Location $Stage
        try {
            Invoke-BoundedProcess $Node @('--test', 'app/smoke.test.ts') 300
        } finally { Pop-Location }

        $uninstall = [string] $registration[0].UninstallString
        $uninstall = $uninstall.Trim('"')
        if (!(Test-Path $uninstall -PathType Leaf)) { throw "Uninstaller missing: $uninstall" }
        # NSIS otherwise copies its uninstaller to a child process and exits early.
        Invoke-BoundedProcess $uninstall @('/S', "_?=$install")
        if (Test-Path $app) { throw 'Uninstall left the application executable behind' }
        if (@(Get-Registration 'HKCU').Count) { throw 'Uninstall left per-user registration behind' }
        'Install, reinstall, app smoke tests, and uninstall passed without elevation.' |
            Set-Content (Join-Path $Stage 'success.txt')
    } catch {
        $_ | Out-String | Set-Content (Join-Path $Stage 'failure.txt')
        throw
    } finally { Stop-Transcript | Out-Null }
    exit 0
}

$installer = @(Get-ChildItem 'target/x86_64-pc-windows-msvc/release/bundle/nsis/*-setup.exe')
if ($installer.Count -ne 1) { throw 'Expected exactly one Windows NSIS installer' }
$userName = 'bb' + [guid]::NewGuid().ToString('N').Substring(0, 12)
$Stage = Join-Path $env:PUBLIC "BranchboardInstaller-$userName"
$diagnostics = Join-Path $PWD 'e2e-results/installer'
$createdUser = $false
try {
    New-Item -ItemType Directory -Path $Stage, $diagnostics -Force | Out-Null
    $password = ConvertTo-SecureString ([guid]::NewGuid().ToString('N') + '!aA9') -AsPlainText -Force
    $user = New-LocalUser -Name $userName -Password $password -AccountNeverExpires -PasswordNeverExpires
    $createdUser = $true
    # Resolve the built-in Users group by SID, independently of runner language.
    $users = Get-LocalGroup -SID 'S-1-5-32-545'
    Add-LocalGroupMember -Group $users -Member $user
    $acl = Get-Acl $Stage
    $rule = [Security.AccessControl.FileSystemAccessRule]::new(
        $user.SID, 'Modify', 'ContainerInherit,ObjectInherit', 'None', 'Allow'
    )
    $acl.AddAccessRule($rule)
    Set-Acl -Path $Stage -AclObject $acl
    Copy-Item $installer[0].FullName (Join-Path $Stage 'Branchboard-setup.exe')
    Copy-Item (Get-Command tauri-driver.exe).Source (Join-Path $Stage 'tauri-driver.exe')
    Copy-Item $env:NATIVE_DRIVER (Join-Path $Stage 'msedgedriver.exe')
    Copy-Item 'ui/e2e/app' (Join-Path $Stage 'app') -Recurse
    Copy-Item $PSCommandPath (Join-Path $Stage 'verify.ps1')
    $Node = (Get-Command node.exe).Source
    $credentials = [pscredential]::new("$env:COMPUTERNAME\$userName", $password)
    $arguments = @('-NoProfile', '-NonInteractive', '-File', "`"$Stage\verify.ps1`"", '-AsStandardUser', '-Stage', "`"$Stage`"", '-Node', "`"$Node`"")
    $process = Start-Process -FilePath (Get-Command pwsh.exe).Source -Credential $credentials `
        -LoadUserProfile -ArgumentList $arguments -WorkingDirectory $Stage -PassThru `
        -RedirectStandardOutput (Join-Path $Stage 'stdout.log') `
        -RedirectStandardError (Join-Path $Stage 'stderr.log')
    $null = $process.Handle
    if (!$process.WaitForExit(600000)) {
        & taskkill.exe /PID $process.Id /T /F | Out-Null
        throw 'Standard-user installer verification timed out after 10 minutes'
    }
    if ($process.ExitCode -ne 0 -or !(Test-Path (Join-Path $Stage 'success.txt'))) {
        throw "Standard-user installer verification failed (exit $($process.ExitCode)); see release diagnostics"
    }
    Get-Content (Join-Path $Stage 'success.txt')
} finally {
    if (Test-Path $Stage) {
        foreach ($item in @('standard-user.log', 'stdout.log', 'stderr.log', 'success.txt', 'failure.txt', 'e2e-results')) {
            $path = Join-Path $Stage $item
            if (Test-Path $path) { Copy-Item $path $diagnostics -Recurse -Force }
        }
    }
    if ($createdUser) {
        # Also clean up any descendants left behind on a failed smoke test.
        Get-CimInstance Win32_Process | ForEach-Object {
            $owner = Invoke-CimMethod -InputObject $_ -MethodName GetOwner -ErrorAction SilentlyContinue
            if ($owner -and $owner.User -eq $userName) {
                Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue
            }
        }
        $userProfile = Get-CimInstance Win32_UserProfile | Where-Object { $_.SID -eq $user.SID.Value }
        if ($userProfile) { $userProfile | Remove-CimInstance }
        Remove-LocalUser -Name $userName
    }
    if (Test-Path $Stage) { Remove-Item $Stage -Recurse -Force }
}

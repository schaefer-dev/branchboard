import { spawnSync } from "node:child_process";
import { win32 } from "node:path";
import { setTimeout as delay } from "node:timers/promises";

// Input travels over stdin, never through interpolated PowerShell source.
const script = String.raw`
$ErrorActionPreference = 'Stop'
$request = [Console]::In.ReadToEnd() | ConvertFrom-Json
$base = [Microsoft.Win32.RegistryKey]::OpenBaseKey([Microsoft.Win32.RegistryHive]::LocalMachine, [Microsoft.Win32.RegistryView]::Registry64)
$prefix = 'Software\Policies\Microsoft\Edge\WebView2'
function ProfileMatches($process) {
  if ($process.Name -ine 'msedgewebview2.exe' -or !$process.CommandLine) { return $false }
  $match = [regex]::Match($process.CommandLine, '(?i)(?:^|\s)--user-data-dir=(?:"([^"]+)"|(\S+))(?=\s|$)')
  if (!$match.Success) { return $false }
  $folder = if ($match.Groups[1].Success) { $match.Groups[1].Value } else { $match.Groups[2].Value }
  return [IO.Path]::GetFullPath($folder).TrimEnd('\') -ieq [IO.Path]::GetFullPath($request.profile).TrimEnd('\')
}
try {
  switch ($request.action) {
    'snapshot' {
      $principal = [Security.Principal.WindowsPrincipal]::new([Security.Principal.WindowsIdentity]::GetCurrent())
      $values = @()
      foreach ($setting in @('AdditionalBrowserArguments', 'UserDataFolder')) {
        $path = "$prefix\$setting"
        $key = $base.OpenSubKey($path)
        try {
          $exists = $null -ne $key -and $key.GetValueNames() -contains $request.application
          $value = $null
          $kind = $null
          if ($exists) {
            $kind = $key.GetValueKind($request.application).ToString()
            $value = $key.GetValue($request.application, $null, [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
            if ($kind -eq 'QWord') { $value = $value.ToString([Globalization.CultureInfo]::InvariantCulture) }
          }
          $values += @{ path = $path; exists = $exists; value = $value; kind = $kind }
        } finally { if ($key) { $key.Dispose() } }
      }
      $createdKeys = @()
      foreach ($path in @('Software\Policies\Microsoft\Edge', $prefix, "$prefix\AdditionalBrowserArguments", "$prefix\UserDataFolder")) {
        $key = $base.OpenSubKey($path)
        if (!$key) { $createdKeys += $path } else { $key.Dispose() }
      }
      @{ elevated = $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator); values = $values; createdKeys = $createdKeys } | ConvertTo-Json -Depth 10 -Compress
    }
    'configure' {
      foreach ($setting in @('AdditionalBrowserArguments', 'UserDataFolder')) {
        $key = $base.CreateSubKey("$prefix\$setting")
        try {
          $value = if ($setting -eq 'UserDataFolder') { $request.profile } else { $request.browserArguments }
          $key.SetValue($request.application, [string]$value, [Microsoft.Win32.RegistryValueKind]::String)
        } finally { $key.Dispose() }
      }
    }
    'restore' {
      $errors = @()
      foreach ($saved in $request.snapshot.values) {
        try {
          $key = $base.OpenSubKey($saved.path, $true)
          if (!$key) {
            if (!$saved.exists) { continue }
            $key = $base.CreateSubKey($saved.path)
          }
          try {
            if ($saved.exists) {
              $kind = [Enum]::Parse([Microsoft.Win32.RegistryValueKind], [string]$saved.kind)
              $value = switch ($saved.kind) {
                'DWord' { [int]$saved.value }
                'QWord' { [long]$saved.value }
                'Binary' { ,([byte[]]$saved.value) }
                'MultiString' { ,([string[]]$saved.value) }
                default { [string]$saved.value }
              }
              $key.SetValue($request.application, $value, $kind)
            } else { $key.DeleteValue($request.application, $false) }
          } finally { $key.Dispose() }
        } catch { $errors += $_.Exception.Message }
      }
      $paths = @($request.snapshot.createdKeys)
      [array]::Reverse($paths)
      foreach ($path in $paths) {
        try {
          $key = $base.OpenSubKey($path)
          $empty = $key -and $key.ValueCount -eq 0 -and $key.SubKeyCount -eq 0
          if ($key) { $key.Dispose() }
          if ($empty) { $base.DeleteSubKey($path, $false) }
        } catch { $errors += $_.Exception.Message }
      }
      if ($errors.Count) { throw ($errors -join '; ') }
    }
    'processes' {
      $processes = @(Get-CimInstance Win32_Process -Filter "Name = 'msedgewebview2.exe'" | ForEach-Object {
        @{ pid = [int]$_.ProcessId; parentPid = [int]$_.ParentProcessId; name = $_.Name; commandLine = $_.CommandLine }
      })
      ConvertTo-Json -InputObject $processes -Depth 5 -Compress
    }
    'kill' {
      $process = Get-CimInstance Win32_Process -Filter "ProcessId = $($request.pid)"
      if ($process -and (ProfileMatches $process)) {
        & taskkill.exe /PID $request.pid /T /F | Out-Null
        if ($LASTEXITCODE -ne 0) {
          $remaining = Get-CimInstance Win32_Process -Filter "ProcessId = $($request.pid)"
          if ($remaining -and (ProfileMatches $remaining)) { throw "Could not terminate profile process $($request.pid)" }
        }
      }
    }
    default { throw 'Unknown Windows smoke operation' }
  }
} finally { $base.Dispose() }
`;

export type WindowsCommand = (
  action: string,
  payload: Record<string, unknown>,
  timeoutMs?: number,
) => unknown;

const command: WindowsCommand = (action, payload, timeoutMs = 10_000) => {
  const result = spawnSync("pwsh.exe", ["-NoProfile", "-NonInteractive", "-Command", script], {
    input: JSON.stringify({ ...payload, action }),
    encoding: "utf8",
    timeout: timeoutMs,
    windowsHide: true,
  });
  if (result.error || result.status !== 0) {
    throw new Error(`Windows smoke ${action} failed: ${result.error?.message ?? result.stderr}`);
  }
  return result.stdout.trim() ? (JSON.parse(result.stdout) as unknown) : undefined;
};

interface Snapshot {
  elevated: boolean;
  values: unknown[];
  createdKeys: string[];
}

export interface BrowserProcess {
  pid: number;
  parentPid: number;
  name: string;
  commandLine: string | null;
}

export function matchesProfile(process: BrowserProcess, profile: string): boolean {
  if (process.name.toLowerCase() !== "msedgewebview2.exe" || !process.commandLine) return false;
  const match = /(?:^|\s)--user-data-dir=(?:"([^"]+)"|(\S+))(?=\s|$)/i.exec(process.commandLine);
  const folder = match?.[1] ?? match?.[2];
  const normalize = (path: string) => win32.resolve(path).replace(/\\+$/, "").toLowerCase();
  return !!folder && normalize(folder) === normalize(profile);
}

/** Owns only the app-specific policy values and browser processes for this unique profile. */
export class WindowsWebView2 {
  private readonly run: WindowsCommand;
  private readonly record: (event: string, data: unknown) => void;
  private readonly payload: Record<string, unknown>;
  private snapshot: Snapshot | undefined;

  constructor(
    application: string,
    profile: string,
    browserArguments: string,
    record: (event: string, data: unknown) => void,
    run: WindowsCommand = command,
  ) {
    this.run = run;
    this.record = record;
    this.payload = { application: win32.basename(application), profile, browserArguments };
  }

  prepare(env: NodeJS.ProcessEnv): NodeJS.ProcessEnv {
    this.snapshot = this.run("snapshot", this.payload) as Snapshot;
    this.record("policy-snapshot", this.snapshot);
    const childEnv = { ...env };
    if (!this.snapshot.elevated) {
      childEnv.WEBVIEW2_USER_DATA_FOLDER = String(this.payload.profile);
      childEnv.WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = String(this.payload.browserArguments);
      return childEnv;
    }
    try {
      this.run("configure", this.payload);
      this.record("policy-configured", this.payload);
    } catch (error) {
      try {
        this.restore();
      } catch (restoreError) {
        throw new AggregateError(
          [error, restoreError],
          "Windows policy setup and rollback failed",
          { cause: restoreError },
        );
      }
      throw error;
    }
    return Object.fromEntries(
      Object.entries(childEnv).filter(
        ([key]) => !/^WEBVIEW2_(USER_DATA_FOLDER|ADDITIONAL_BROWSER_ARGUMENTS)$/i.test(key),
      ),
    );
  }

  captureProcesses(timeoutMs = 10_000): BrowserProcess[] {
    const processes = this.run("processes", this.payload, timeoutMs) as BrowserProcess[];
    const owned = processes.filter((process) =>
      matchesProfile(process, String(this.payload.profile)),
    );
    this.record("profile-processes", owned);
    return owned;
  }

  async stopProfileProcesses(timeoutMs = 5_000): Promise<void> {
    const deadline = Date.now() + timeoutMs;
    const remaining = () => Math.max(1, deadline - Date.now());
    while (Date.now() < deadline) {
      const processes = this.captureProcesses(remaining());
      if (!processes.length) {
        this.record("profile-processes-stopped", true);
        return;
      }
      const ownedPids = new Set(processes.map((process) => process.pid));
      // Killing a root tree also stops its children; avoid redundant commands against exited PIDs.
      for (const process of processes.filter((process) => !ownedPids.has(process.parentPid))) {
        if (Date.now() >= deadline) break;
        this.run("kill", { ...this.payload, pid: process.pid }, remaining());
      }
      await delay(Math.min(100, Math.max(0, deadline - Date.now())));
    }
    throw new Error(
      `Profile processes did not terminate within ${String(timeoutMs)}ms; temporary data was retained`,
    );
  }

  restore(): void {
    if (!this.snapshot?.elevated) return;
    this.run("restore", { ...this.payload, snapshot: this.snapshot });
    this.record("policy-restored", true);
    this.snapshot = undefined;
  }
}

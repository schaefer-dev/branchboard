<script lang="ts">
  import type { AppErrorKind } from "../../lib/api";
  import { appInfo } from "../../lib/state/app-info.svelte";

  const gitErrorText: Record<AppErrorKind, string> = {
    gitNotFound: "Git not found. Install git and restart Branchboard.",
    gitUnsupported: "This git version is too old. Update git to 2.41 or newer.",
    gitFailed: "Git could not be started.",
    internal: "Git status unavailable.",
  };

  const git = $derived(appInfo.git);
  const gitError = $derived(appInfo.gitError);
</script>

<footer class="statusbar">
  <span class="git">
    {#if git}
      <span title={git.path}>
        git {git.version.major}.{git.version.minor}.{git.version.patch}
      </span>
    {:else if gitError}
      <span class="error" title={gitError.detail ?? gitError.message}>
        {gitErrorText[gitError.kind]}
      </span>
    {:else}
      <span class="pending">Looking for git</span>
    {/if}
  </span>
  {#if appInfo.app}
    <span class="version">Branchboard {appInfo.app.version}</span>
  {/if}
</footer>

<style>
  .statusbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-4);
    height: var(--statusbar-height);
    padding: 0 var(--space-4);
    border-top: 1px solid var(--color-border);
    background: var(--color-chrome);
    color: var(--color-text-muted);
    font-size: var(--text-xs);
    font-variant-numeric: tabular-nums;
  }

  .error {
    color: var(--color-danger);
  }

  .pending,
  .version {
    color: var(--color-text-faint);
  }
</style>

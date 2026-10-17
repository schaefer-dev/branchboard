<script lang="ts">
  import { appInfo } from "../../lib/state/app-info.svelte";
  import { workspace } from "../../lib/state/workspace.svelte";
  import Icon from "../../lib/ui/Icon.svelte";

  const git = $derived(appInfo.git);
  const gitError = $derived(appInfo.gitError);
  const muted = $derived(workspace.snapshot.settings.soundMuted);
  const muteWhenWatching = $derived(workspace.snapshot.settings.muteWhenWatching);
</script>

<footer class="statusbar">
  <span class="git">
    {#if git}
      <span title={git.path}>
        git {git.version.major}.{git.version.minor}.{git.version.patch}
      </span>
    {:else if gitError}
      <span class="error" title={gitError.detail ?? undefined}>{gitError.message}</span>
    {:else}
      <span class="pending">Looking for git</span>
    {/if}
  </span>
  <span class="end">
    <span class="sounds" role="group" aria-label="Agent sounds">
      <button
        type="button"
        class="toggle"
        aria-label="Mute agent sounds"
        aria-pressed={muted}
        title={muted
          ? "Agent sounds are muted; notifications still show"
          : "Mute the sounds when an agent needs you"}
        onclick={() => void workspace.setSoundMuted(!muted)}
      >
        <Icon name={muted ? "bell-off" : "bell"} size={14} />
      </button>
      <button
        type="button"
        class="toggle"
        aria-label="Mute the agent on screen"
        aria-pressed={muteWhenWatching}
        disabled={muted}
        title={muteWhenWatching
          ? "No sound for an agent you are watching in the terminal"
          : "Sounds play even for an agent you are watching in the terminal"}
        onclick={() => void workspace.setMuteWhenWatching(!muteWhenWatching)}
      >
        <Icon name="eye" size={14} />
      </button>
    </span>
    {#if appInfo.app}
      <span class="version">Branchboard {appInfo.app.version}</span>
    {/if}
  </span>
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

  .end {
    display: flex;
    align-items: center;
    gap: var(--space-4);
  }

  .sounds {
    display: flex;
    gap: var(--space-1);
  }

  .toggle {
    display: inline-grid;
    place-items: center;
    width: 20px;
    height: 18px;
    padding: 0;
    border: 0;
    border-radius: var(--radius-sm);
    background: none;
    color: var(--color-text-faint);
  }

  .toggle:hover {
    background: var(--color-row-hover);
    color: var(--color-text);
  }

  .toggle[aria-pressed="true"] {
    color: var(--color-text);
  }

  .toggle:disabled {
    opacity: 0.45;
    pointer-events: none;
  }
</style>

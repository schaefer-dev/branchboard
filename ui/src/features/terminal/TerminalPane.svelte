<script lang="ts">
  import { FitAddon } from "@xterm/addon-fit";
  import { Unicode11Addon } from "@xterm/addon-unicode11";
  import { WebglAddon } from "@xterm/addon-webgl";
  import { Terminal } from "@xterm/xterm";
  import "@xterm/xterm/css/xterm.css";
  import { onMount, untrack } from "svelte";

  import type { RepoView } from "../../lib/api";
  import { isMac } from "../../lib/platform/os";
  import { terminal } from "../../lib/state/terminal.svelte";
  import { view } from "../../lib/state/view.svelte";
  import { workspace } from "../../lib/state/workspace.svelte";
  import EmptyState from "../../lib/ui/EmptyState.svelte";
  import PaneHeader from "../../lib/ui/PaneHeader.svelte";
  import { openRepoInHerdr } from "../agents/open-in-herdr";
  import RightSwitch from "../shell/RightSwitch.svelte";
  import { isCopy, isNativePasteShortcut, keyOwner } from "./key-policy";
  import { terminalFont, terminalTheme } from "./terminal-theme";

  interface Props {
    /** The pane is on screen; it sizes the terminal only then. */
    visible: boolean;
    /** The selected repository, if herdr has no workspace for it yet. */
    withoutWorkspace: RepoView | null;
  }

  let { visible, withoutWorkspace }: Props = $props();

  let host = $state<HTMLElement>();
  let term: Terminal | null = null;
  let fit: FitAddon | null = null;

  /** Fits the terminal to the pane and tells the backend about new sizes. */
  function fitToPane() {
    if (!term || !fit || !visible) return;
    const before = { cols: term.cols, rows: term.rows };
    fit.fit();
    if (term.cols !== before.cols || term.rows !== before.rows) {
      terminal.resize(term.cols, term.rows);
    }
  }

  function start() {
    if (!term) return;
    term.reset();
    const output = term;
    void terminal.start(term.cols, term.rows, (data) => {
      output.write(data);
    });
  }

  onMount(() => {
    if (!host) return;
    const mac = isMac;
    const instance = new Terminal({
      allowProposedApi: true,
      // herdr keeps the scrollback of every pane itself.
      scrollback: 0,
      fontFamily: terminalFont(),
      fontSize: untrack(() => workspace.snapshot.settings.terminalFontSize),
      theme: terminalTheme(),
      // Option composes characters (e.g. @ and [ on German layouts).
      macOptionIsMeta: false,
      // ConPTY swallows herdr's kitty keyboard request and asks for
      // win32-input-mode instead; without it Shift+Enter arrives as Enter.
      vtExtensions: { kittyKeyboard: true, win32InputMode: true },
    });
    const fitAddon = new FitAddon();
    instance.loadAddon(fitAddon);
    instance.loadAddon(new Unicode11Addon());
    instance.unicode.activeVersion = "11";
    instance.attachCustomKeyEventHandler((event) => {
      // Keep the browser's paste action and Windows clipboard history. Do
      // this for key releases too, which Kitty can otherwise send as input.
      if (isNativePasteShortcut(event, mac)) return false;
      if (event.type === "keydown" && isCopy(event, mac)) {
        const selection = instance.getSelection();
        if (selection) void navigator.clipboard.writeText(selection);
        event.preventDefault();
        return false;
      }
      // Keys for the app bubble to its shortcut handler instead.
      return keyOwner(event, mac) === "terminal";
    });
    instance.onData((data) => {
      terminal.write(data);
    });
    instance.open(host);
    // On macOS the DOM renderer lets WebKit draw text with CoreText, as
    // native terminals do (heavier, smoother strokes than WebGL's glyph
    // atlas). Elsewhere WebGL is faster and looks the same.
    if (!mac) {
      try {
        const webgl = new WebglAddon();
        webgl.onContextLoss(() => {
          webgl.dispose();
        });
        instance.loadAddon(webgl);
      } catch {
        // The DOM renderer stays in place.
      }
    }
    term = instance;
    fit = fitAddon;
    if (import.meta.env.MODE === "e2e") window.__terminal = instance;

    fitToPane();
    start();

    const observer = new ResizeObserver(() => {
      requestAnimationFrame(fitToPane);
    });
    observer.observe(host);
    const scheme = matchMedia("(prefers-color-scheme: dark)");
    const retheme = () => {
      instance.options.theme = terminalTheme();
    };
    scheme.addEventListener("change", retheme);

    return () => {
      observer.disconnect();
      scheme.removeEventListener("change", retheme);
      instance.dispose();
      term = null;
      fit = null;
    };
  });

  $effect(() => {
    if (visible) requestAnimationFrame(fitToPane);
  });

  // A new font size changes how many cells fit; herdr redraws for the new size.
  $effect(() => {
    const size = workspace.snapshot.settings.terminalFontSize;
    if (!term || term.options.fontSize === size) return;
    term.options.fontSize = size;
    requestAnimationFrame(fitToPane);
  });

  $effect(() => {
    if (view.terminalFocusRequest > 0 && visible) term?.focus();
  });
</script>

<section class="terminal" aria-label="Terminal">
  <PaneHeader>
    <span class="title">herdr</span>
    <RightSwitch />
  </PaneHeader>
  {#if withoutWorkspace}
    {@const repo = withoutWorkspace}
    <div class="banner" role="status">
      <span>{repo.name} has no herdr workspace yet.</span>
      <button type="button" class="btn" onclick={() => void openRepoInHerdr(repo)}>
        Create workspace
      </button>
    </div>
  {/if}
  <div class="body">
    <div class="host" bind:this={host}></div>
    {#if terminal.status === "exited" || terminal.status === "failed"}
      <div class="overlay">
        <EmptyState
          title={terminal.status === "exited"
            ? "herdr closed"
            : (terminal.error?.message ?? "The terminal could not be started.")}
          hint={terminal.status === "exited"
            ? "Your agents keep running in herdr. Reattach to see them again."
            : (terminal.error?.detail ?? undefined)}
        />
        <button type="button" class="btn btn-primary" onclick={start}>
          {terminal.status === "exited" ? "Reattach" : "Try again"}
        </button>
      </div>
    {/if}
  </div>
</section>

<style>
  .terminal {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    background: var(--color-surface);
  }

  .title {
    color: var(--color-text-muted);
    font-weight: 600;
  }

  .banner {
    display: flex;
    align-items: center;
    gap: var(--space-4);
    flex: none;
    padding: var(--space-2) var(--space-4);
    border-bottom: 1px solid var(--color-border);
    background: var(--color-sunken);
    color: var(--color-text-muted);
    font-size: var(--text-sm);
  }

  .body {
    position: relative;
    flex: 1;
    min-height: 0;
  }

  .host {
    position: absolute;
    /* The app uses thin "antialiased" text; terminal text keeps the
       system's font smoothing like native terminals. */
    -webkit-font-smoothing: auto;
    inset: var(--space-2) 0 0 var(--space-3);
  }

  .overlay {
    position: absolute;
    /* Above xterm's canvas layers. */
    z-index: 10;
    inset: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--space-4);
    background: var(--color-surface);
  }
</style>

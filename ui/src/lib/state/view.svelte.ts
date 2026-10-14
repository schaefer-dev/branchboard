export type WorkList = "changes" | "branches";

/** What the right column shows: the diff or commits of `detail`, or the terminal. */
export type RightPane = "detail" | "terminal";

/** Layout of the middle pane and which of its lists the right pane follows. */
class ViewStore {
  /** The list used last; the right pane shows its diff or commits. */
  detail = $state<WorkList>("changes");
  /** Height of the branch list set with the splitter; `null` fits up to eight rows. Session only. */
  branchHeight = $state<number | null>(null);
  /** Height of the agents list set with the splitter; `null` fits up to five agents. Session only. */
  agentsHeight = $state<number | null>(null);
  /** Heights of the review's commit and file lists set with the splitters; `null` fits them. Session only. */
  reviewCommitsHeight = $state<number | null>(null);
  reviewFilesHeight = $state<number | null>(null);
  right = $state<RightPane>("detail");
  /** Asks the terminal to take keyboard focus; counts up per request. */
  terminalFocusRequest = $state(0);
  /** Asks a list to take keyboard focus; `seq` makes repeated requests distinct. */
  focusRequest = $state<{ list: WorkList; seq: number } | null>(null);

  /** Shows the terminal in the right column and gives it the keyboard. */
  showTerminal(): void {
    this.right = "terminal";
    this.terminalFocusRequest += 1;
  }

  focus(list: WorkList): void {
    this.detail = list;
    this.right = "detail";
    this.focusRequest = { list, seq: (this.focusRequest?.seq ?? 0) + 1 };
  }
}

export const view = new ViewStore();

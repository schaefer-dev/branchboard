export type WorkList = "changes" | "branches";

/** Layout of the middle pane and which of its lists the right pane follows. */
class ViewStore {
  /** The list used last; the right pane shows its diff or commits. */
  detail = $state<WorkList>("changes");
  /** Height of the branch list set with the splitter; `null` fits up to eight rows. Session only. */
  branchHeight = $state<number | null>(null);
  /** Asks a list to take keyboard focus; `seq` makes repeated requests distinct. */
  focusRequest = $state<{ list: WorkList; seq: number } | null>(null);

  focus(list: WorkList): void {
    this.detail = list;
    this.focusRequest = { list, seq: (this.focusRequest?.seq ?? 0) + 1 };
  }
}

export const view = new ViewStore();

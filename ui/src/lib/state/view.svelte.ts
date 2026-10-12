export type WorkView = "changes" | "branches";

/** Which view the middle pane shows. */
class ViewStore {
  work = $state<WorkView>("changes");
}

export const view = new ViewStore();

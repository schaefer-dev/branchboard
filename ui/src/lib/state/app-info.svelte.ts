import { api, CommandError, toAppError, type AppError, type AppInfo, type GitInfo } from "../api";

/** Static information about the app and the git installation it uses. */
export class AppInfoStore {
  app = $state<AppInfo | null>(null);
  git = $state<GitInfo | null>(null);
  gitError = $state<AppError | null>(null);

  async load(): Promise<void> {
    const [app, git] = await Promise.allSettled([api.getAppInfo(), api.getGitInfo()]);
    this.app = app.status === "fulfilled" ? app.value : null;
    if (git.status === "fulfilled") {
      this.git = git.value;
      this.gitError = null;
    } else {
      this.git = null;
      this.gitError =
        git.reason instanceof CommandError ? git.reason.error : toAppError(git.reason);
    }
  }
}

export const appInfo = new AppInfoStore();

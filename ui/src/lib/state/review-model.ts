// What the review pane can show for a branch: all of its changes against the
// mainline, or one of its commits.

import type { BranchCommits, ReviewTarget } from "../api";

export function targetKey(target: ReviewTarget): string {
  return target.kind === "branch" ? `branch:${target.name}` : `commit:${target.oid}`;
}

/** The branch's mainline exists, so its changes can be reviewed as a whole. */
export function canReviewWhole(commits: BranchCommits): boolean {
  return commits.behind !== null;
}

/** Every target in display order: the whole branch, then ahead, then behind commits. */
export function reviewTargets(commits: BranchCommits): ReviewTarget[] {
  const whole: ReviewTarget[] = canReviewWhole(commits)
    ? [{ kind: "branch", name: commits.branch }]
    : [];
  const listed = [...commits.ahead.commits, ...(commits.behind?.commits ?? [])];
  return [...whole, ...listed.map((commit): ReviewTarget => ({ kind: "commit", oid: commit.oid }))];
}

/** Keeps `current` if `commits` still offers it, else the first target. */
export function chooseTarget(
  commits: BranchCommits,
  current: ReviewTarget | null,
): ReviewTarget | null {
  const targets = reviewTargets(commits);
  const key = current ? targetKey(current) : null;
  return targets.find((target) => targetKey(target) === key) ?? targets[0] ?? null;
}

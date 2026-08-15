function names(root: string, args: string[]): string[] {
  const proc = Bun.spawnSync(["git", ...args], { cwd: root, stdout: "pipe", stderr: "pipe" });
  if (proc.exitCode !== 0) throw new Error(`git ${args.join(" ")} failed: ${proc.stderr.toString()}`);
  return proc.stdout.toString().split("\n").map(value => value.trim()).filter(Boolean);
}

export function completeFixtureChanges(root: string, baseline: string): string[] {
  return [...new Set([
    ...names(root, ["diff", "--name-only", `${baseline}..HEAD`]),
    ...names(root, ["diff", "--name-only"]),
    ...names(root, ["diff", "--cached", "--name-only"]),
    ...names(root, ["ls-files", "--others", "--exclude-standard"]),
  ])].sort();
}

export function receiptMatchesChanges(receipt: any, changed: string[]): boolean {
  if (!Array.isArray(receipt?.changed_files) || !receipt.changed_files.every((value: unknown) => typeof value === "string")) return false;
  const claimed = [...new Set(receipt.changed_files as string[])].sort();
  return claimed.length === changed.length && claimed.every((value, index) => value === changed[index]);
}

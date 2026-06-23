import { execFileSync } from "child_process";

export type VerifyStatus =
  | "ok"
  | "bad-signature"
  | "source-missing"
  | "turn-missing"
  | "source-modified"
  | "span-mismatch"
  | "not-found"
  | "error";

const STATUS_ICON: Record<VerifyStatus, string> = {
  ok: "✓",
  "bad-signature": "✗",
  "source-missing": "⚠",
  "turn-missing": "⚠",
  "source-modified": "⚠",
  "span-mismatch": "⚠",
  "not-found": "?",
  error: "!",
};

const STATUS_CLASS: Record<VerifyStatus, string> = {
  ok: "muginn-ok",
  "bad-signature": "muginn-bad",
  "source-missing": "muginn-warn",
  "turn-missing": "muginn-warn",
  "source-modified": "muginn-warn",
  "span-mismatch": "muginn-warn",
  "not-found": "muginn-unknown",
  error: "muginn-error",
};

export function runVerify(muginnBin: string, atomId: string): VerifyStatus {
  try {
    // `muginn verify <id>` prints a single status string. execFileSync passes argv
    // directly — no shell — so the atom id cannot inject shell commands.
    const out = execFileSync(muginnBin, ["verify", atomId], {
      encoding: "utf8",
      timeout: 5000,
    });
    const s = out.trim().split("\n").pop()?.trim() as VerifyStatus;
    if (!s) return "not-found";
    return STATUS_ICON[s] ? s : "error";
  } catch {
    return "error";
  }
}

export function statusIcon(s: VerifyStatus): string {
  return STATUS_ICON[s] ?? "!";
}

export function statusClass(s: VerifyStatus): string {
  return STATUS_CLASS[s] ?? "muginn-error";
}

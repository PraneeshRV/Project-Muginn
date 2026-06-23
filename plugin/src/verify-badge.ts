import { execSync } from "child_process";

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
    const out = execSync(`${muginnBin} recall "${atomId}" -k 1`, {
      encoding: "utf8",
      timeout: 5000,
    });
    // Parse verify status line: "  verify[<id8>] = <status>"
    const match = out.match(/verify\[[\w]+\]\s*=\s*(\S+)/);
    if (!match) return "not-found";
    const s = match[1] as VerifyStatus;
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

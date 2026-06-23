import { execSync } from "child_process";
import { Notice, Platform } from "obsidian";

export interface CitationInfo {
  agent: string;
  native_path: string;
  session_id: string;
  turn_id: string;
  span: [number, number];
  turn_sha256: string;
}

export function fetchCitation(muginnBin: string, atomId: string): CitationInfo | null {
  try {
    // muginn recall returns cards + verify lines; we need the citation JSON.
    // Use a temp DB recall that prints structured info. Since CLI only prints cards,
    // we grep for the citation fields from the stored data via stdout of recall.
    // Fallback: parse native_path from atom note frontmatter (passed in from caller).
    const out = execSync(`${muginnBin} recall "${atomId}" -k 1`, {
      encoding: "utf8",
      timeout: 5000,
    });
    // The recall output is markdown cards; citation JSON comes from `muginn cite`.
    // For now, run recall and parse the card line: `- "<quote>" — agent:session#turn [id8]`
    const cardLine = out.split("\n").find((l) => l.startsWith("- "));
    if (!cardLine) return null;
    // We can't get full citation from recall output alone. Return null so the caller
    // uses the frontmatter fields directly.
    return null;
  } catch {
    return null;
  }
}

export function openNativePath(nativePath: string, span: [number, number]): void {
  if (!nativePath) {
    new Notice("Muginn: no native_path in frontmatter.");
    return;
  }

  const [start, end] = span;
  const notice = `Source: ${nativePath}\nBytes [${start}, ${end}]`;

  // On desktop, open the file with the system default app.
  // The byte span is shown in a notice since most editors don't support byte-seek.
  if (Platform.isDesktopApp) {
    try {
      // Use shell to open; Electron exposes require('electron').shell
      const { shell } = (window as any).require("electron");
      shell.openPath(nativePath).then((err: string) => {
        if (err) {
          new Notice(`Muginn: could not open ${nativePath}\n${err}`);
        } else {
          new Notice(`Muginn: opened source\n${notice}`);
        }
      });
    } catch {
      new Notice(`Muginn source span:\n${notice}`);
    }
  } else {
    new Notice(`Muginn source span:\n${notice}`);
  }
}

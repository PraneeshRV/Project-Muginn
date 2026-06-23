import { execFileSync } from "child_process";
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
  if (!atomId) return null;
  try {
    // `muginn cite <id>` prints the citation JSON. execFileSync passes argv directly —
    // no shell — so the atom id cannot inject shell commands.
    const out = execFileSync(muginnBin, ["cite", atomId], {
      encoding: "utf8",
      timeout: 5000,
    }).trim();
    const json = JSON.parse(out);
    if (json.error || typeof json.native_path !== "string") return null;
    return json as CitationInfo;
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

import { App, Notice, Plugin, StatusBarItem, TFile, parseYaml } from "obsidian";
import { DEFAULT_SETTINGS, MuginnSettings, MuginnSettingTab } from "./settings";
import { runVerify, statusIcon, statusClass } from "./verify-badge";
import { openNativePath } from "./source-jump";
import { execSync } from "child_process";

export default class MuginnPlugin extends Plugin {
  settings: MuginnSettings;
  private statusBar: StatusBarItem;

  async onload() {
    await this.loadSettings();

    this.statusBar = this.addStatusBarItem();
    this.statusBar.setText("");

    this.addSettingTab(new MuginnSettingTab(this.app, this));

    // ── Verify atom command ───────────────────────────────────────────────
    this.addCommand({
      id: "verify-atom",
      name: "Verify atom",
      callback: () => this.verifyActiveNote(),
    });

    // ── Source jump command ───────────────────────────────────────────────
    this.addCommand({
      id: "jump-to-source",
      name: "Jump to source span",
      callback: () => this.jumpToSource(),
    });

    // ── Recompile page command ────────────────────────────────────────────
    this.addCommand({
      id: "recompile-page",
      name: "Recompile page",
      callback: () => this.recompilePage(),
    });

    // ── Auto-verify on file open ──────────────────────────────────────────
    this.registerEvent(
      this.app.workspace.on("file-open", (file) => {
        if (file && this.settings.autoVerifyOnOpen) {
          this.updateBadgeForFile(file);
        }
      })
    );
  }

  onunload() {
    this.statusBar.setText("");
  }

  async loadSettings() {
    this.settings = Object.assign({}, DEFAULT_SETTINGS, await this.loadData());
  }

  async saveSettings() {
    await this.saveData(this.settings);
  }

  private async readFrontmatter(file: TFile): Promise<Record<string, any> | null> {
    const content = await this.app.vault.read(file);
    const match = content.match(/^---\n([\s\S]*?)\n---/);
    if (!match) return null;
    try {
      return parseYaml(match[1]) ?? null;
    } catch {
      return null;
    }
  }

  private async updateBadgeForFile(file: TFile) {
    const fm = await this.readFrontmatter(file);
    if (!fm) {
      this.statusBar.setText("");
      return;
    }

    const atomId: string | undefined = fm["atom_id"];
    const coverage: number | undefined = fm["coverage"];

    if (atomId) {
      // Atom note: show verify badge
      const status = runVerify(this.settings.muginnBin, atomId);
      const icon = statusIcon(status);
      const cls = statusClass(status);
      this.statusBar.setText(`Muginn ${icon} ${status}`);
      this.statusBar.addClass(cls);
    } else if (coverage !== undefined) {
      // Page note: show coverage
      const pct = (coverage * 100).toFixed(0);
      const cls = coverage >= 0.95 ? "muginn-ok" : "muginn-warn";
      this.statusBar.setText(`Muginn coverage ${pct}%`);
      this.statusBar.addClass(cls);
    } else {
      this.statusBar.setText("");
    }
  }

  private async verifyActiveNote() {
    const file = this.app.workspace.getActiveFile();
    if (!file) {
      new Notice("Muginn: no active file.");
      return;
    }
    const fm = await this.readFrontmatter(file);
    const atomId: string | undefined = fm?.["atom_id"];
    if (!atomId) {
      new Notice("Muginn: no atom_id in frontmatter.");
      return;
    }
    new Notice("Muginn: verifying…");
    const status = runVerify(this.settings.muginnBin, atomId);
    const icon = statusIcon(status);
    new Notice(`Muginn verify: ${icon} ${status}`);
    await this.updateBadgeForFile(file);
  }

  private async jumpToSource() {
    const file = this.app.workspace.getActiveFile();
    if (!file) {
      new Notice("Muginn: no active file.");
      return;
    }
    const fm = await this.readFrontmatter(file);
    if (!fm || !fm["atom_id"]) {
      new Notice("Muginn: no atom_id in frontmatter.");
      return;
    }

    const nativePath: string = fm["native_path"] ?? "";
    const spanRaw = fm["span"];
    const span: [number, number] = Array.isArray(spanRaw)
      ? [Number(spanRaw[0]), Number(spanRaw[1])]
      : [0, 0];

    openNativePath(nativePath, span);
  }

  private async recompilePage() {
    const file = this.app.workspace.getActiveFile();
    if (!file) {
      new Notice("Muginn: no active file.");
      return;
    }
    const fm = await this.readFrontmatter(file);
    // Page notes have a topic derived from the file name
    const topic = fm?.["topic"] ?? file.basename;
    new Notice(`Muginn: recompiling "${topic}"…`);
    try {
      const out = execSync(
        `${this.settings.muginnBin} compile "${topic}"`,
        { encoding: "utf8", timeout: 30000 }
      );
      new Notice(`Muginn: ${out.trim().split("\n")[0]}`);
    } catch (e: any) {
      new Notice(`Muginn: recompile failed — ${e.message}`);
    }
  }
}

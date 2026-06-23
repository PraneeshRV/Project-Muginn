import { App, PluginSettingTab, Setting } from "obsidian";
import type MuginnPlugin from "./main";

export interface MuginnSettings {
  muginnBin: string;
  autoVerifyOnOpen: boolean;
}

export const DEFAULT_SETTINGS: MuginnSettings = {
  muginnBin: "muginn",
  autoVerifyOnOpen: true,
};

export class MuginnSettingTab extends PluginSettingTab {
  plugin: MuginnPlugin;

  constructor(app: App, plugin: MuginnPlugin) {
    super(app, plugin);
    this.plugin = plugin;
  }

  display(): void {
    const { containerEl } = this;
    containerEl.empty();

    new Setting(containerEl)
      .setName("Muginn binary")
      .setDesc("Path to the muginn CLI binary (must be on PATH or absolute).")
      .addText((text) =>
        text
          .setPlaceholder("muginn")
          .setValue(this.plugin.settings.muginnBin)
          .onChange(async (value) => {
            this.plugin.settings.muginnBin = value.trim() || "muginn";
            await this.plugin.saveSettings();
          })
      );

    new Setting(containerEl)
      .setName("Auto-verify on file open")
      .setDesc("Run verify and update the status bar badge whenever an atom note is opened.")
      .addToggle((toggle) =>
        toggle
          .setValue(this.plugin.settings.autoVerifyOnOpen)
          .onChange(async (value) => {
            this.plugin.settings.autoVerifyOnOpen = value;
            await this.plugin.saveSettings();
          })
      );
  }
}

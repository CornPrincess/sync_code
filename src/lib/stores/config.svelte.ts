import { loadConfig, saveConfig, defaultAppConfig, type AppConfig } from '../ipc.js';

class ConfigStore {
  value = $state<AppConfig>(defaultAppConfig());
  loaded = $state(false);

  async load() {
    try {
      this.value = await loadConfig();
    } catch {
      this.value = defaultAppConfig();
    }
    this.loaded = true;
  }

  async save() {
    await saveConfig(this.value);
  }
}

export const configStore = new ConfigStore();

import { createSignal, For, Show } from "solid-js";
import { Clock, Palette, Image, Trash2, Upload } from "lucide-solid";
import { timeFormat, setTimeFormat, theme, setTheme, backgroundImage, saveBackground, removeBackground, type Theme } from "../settings";
import "./Settings.css";

const THEMES: { id: Theme; label: string }[] = [
  { id: "dark-red", label: "Dark Red" },
  { id: "dark-blue", label: "Dark Blue" },
  { id: "dark-purple", label: "Dark Purple" },
  { id: "dark-green", label: "Dark Green" },
];

interface SettingsProps {
  onClose: () => void;
}

export function Settings(_props: SettingsProps) {
  const [uploading, setUploading] = createSignal(false);

  async function handleFilePick() {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = "image/*";
    input.onchange = async () => {
      const file = input.files?.[0];
      if (!file) return;
      setUploading(true);
      try {
        const dataUrl = await fileToDataUrl(file);
        await saveBackground(dataUrl);
      } finally {
        setUploading(false);
      }
    };
    input.click();
  }

  return (
    <div class="app-settings">
      <div class="settings-section">
        <div class="settings-section-header">
          <Clock size={16} />
          <span>Time Format</span>
        </div>
        <div class="settings-options">
          <button
            class="settings-option"
            classList={{ active: timeFormat() === "24h" }}
            onClick={() => setTimeFormat("24h")}
          >
            <span class="settings-option-label">24-Hour</span>
            <span class="settings-option-desc">14:30</span>
          </button>
          <button
            class="settings-option"
            classList={{ active: timeFormat() === "12h" }}
            onClick={() => setTimeFormat("12h")}
          >
            <span class="settings-option-label">12-Hour</span>
            <span class="settings-option-desc">2:30 PM</span>
          </button>
        </div>
      </div>

      <div class="settings-section">
        <div class="settings-section-header">
          <Image size={16} />
          <span>Desktop Background</span>
        </div>
        <div class="settings-bg-preview" classList={{ "has-bg": !!backgroundImage() }}>
          <Show when={backgroundImage()}>
            <img src={backgroundImage()!} alt="Background preview" class="settings-bg-img" />
          </Show>
        </div>
        <div class="settings-options">
          <button class="settings-option" onClick={handleFilePick} disabled={uploading()}>
            <Upload size={16} />
            <span class="settings-option-label">{uploading() ? "Uploading..." : "Upload Image"}</span>
          </button>
          <Show when={backgroundImage()}>
            <button class="settings-option" onClick={removeBackground}>
              <Trash2 size={16} />
              <span class="settings-option-label">Remove Background</span>
            </button>
          </Show>
        </div>
      </div>

      <div class="settings-section">
        <div class="settings-section-header">
          <Palette size={16} />
          <span>Color Theme</span>
        </div>
        <div class="settings-options settings-themes">
          <For each={THEMES}>
            {(t) => (
              <button
                class="settings-theme-card"
                classList={{ active: theme() === t.id }}
                onClick={() => setTheme(t.id)}
                data-theme-preview={t.id}
              >
                <div class="theme-swatches">
                  <span class="swatch-primary" />
                  <span class="swatch-secondary" />
                  <span class="swatch-accent" />
                </div>
                <span class="settings-option-label">{t.label}</span>
              </button>
            )}
          </For>
        </div>
      </div>
    </div>
  );
}

function fileToDataUrl(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(reader.result as string);
    reader.onerror = reject;
    reader.readAsDataURL(file);
  });
}

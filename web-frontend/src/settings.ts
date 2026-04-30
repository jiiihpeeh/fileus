import { createRoot, createSignal, createEffect } from "solid-js";
import localforage from "localforage";

export type TimeFormat = "12h" | "24h";
export type Theme = "dark-red" | "dark-blue" | "dark-purple" | "dark-green";

function load<T>(key: string, fallback: T): T {
  try {
    const v = localStorage.getItem(key);
    return v !== null ? (JSON.parse(v) as T) : fallback;
  } catch {
    return fallback;
  }
}

function save<T>(key: string, value: T) {
  localStorage.setItem(key, JSON.stringify(value));
}

export const [timeFormat, setTimeFormat] = createRoot(() => {
  const [tf, setTf] = createSignal<TimeFormat>(load("fileus-time-format", "24h"));
  createEffect(() => save("fileus-time-format", tf()));
  return [tf, setTf] as const;
});

const initialTheme = load("fileus-theme", "dark-red");
document.documentElement.setAttribute("data-theme", initialTheme);

export const [theme, setTheme] = createRoot(() => {
  const [t, setT] = createSignal<Theme>(initialTheme);
  createEffect(() => {
    save("fileus-theme", t());
    document.documentElement.setAttribute("data-theme", t());
  });
  return [t, setT] as const;
});

const bgStore = localforage.createInstance({ name: "fileus", storeName: "background" });

export const [backgroundImage, setBackgroundImage] = createRoot(() => {
  const [bg, setBg] = createSignal<string | null>(null);
  return [bg, setBg] as const;
});

export async function loadBackground() {
  try {
    const data = await bgStore.getItem<string>("image");
    if (data) setBackgroundImage(data);
  } catch {}
}

export async function saveBackground(dataUrl: string) {
  await bgStore.setItem("image", dataUrl);
  setBackgroundImage(dataUrl);
}

export async function removeBackground() {
  await bgStore.removeItem("image");
  setBackgroundImage(null);
}

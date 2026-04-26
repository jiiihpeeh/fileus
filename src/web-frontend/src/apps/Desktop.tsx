import { createSignal, For, Show, onMount, onCleanup } from "solid-js";
import { Window } from "../components/Window";
import { FileBrowser } from "./FileBrowser";
import { Terminal } from "./Terminal";
import { ProcessManager } from "./ProcessManager";
import { TextEditor } from "./TextEditor";
import { ImageViewer } from "./ImageViewer";

export const APP_IDS = {
  FILES: "files",
  TERMINAL: "terminal",
  PROCESSES: "processes",
  EDITOR: "editor",
  IMAGE_VIEWER: "image_viewer",
};

export const APP_META = {
  [APP_IDS.FILES]: { title: "Files", icon: "📁" },
  [APP_IDS.TERMINAL]: { title: "Terminal", icon: "💻" },
  [APP_IDS.PROCESSES]: { title: "Processes", icon: "🔄" },
  [APP_IDS.EDITOR]: { title: "Editor", icon: "📝" },
  [APP_IDS.IMAGE_VIEWER]: { title: "Image Viewer", icon: "🖼️" },
};

interface WindowState {
  id: string;
  minimized: boolean;
}

export function Desktop() {
  const [windows, setWindows] = createSignal<WindowState[]>([]);
  const [startMenuOpen, setStartMenuOpen] = createSignal(false);
  const [clock, setClock] = createSignal(new Date());

  onMount(() => {
    const timer = setInterval(() => setClock(new Date()), 1000);
    onCleanup(() => clearInterval(timer));
  });

  function openApp(appId: string) {
    if (!windows().find(w => w.id === appId)) {
      setWindows(prev => [...prev, { id: appId, minimized: false }]);
    }
    setStartMenuOpen(false);
  }

  function closeWindow(appId: string) {
    setWindows(prev => prev.filter(w => w.id !== appId));
  }

  function minimizeWindow(appId: string) {
    setWindows(prev => prev.map(w => w.id === appId ? { ...w, minimized: true } : w));
  }

  function restoreWindow(appId: string) {
    setWindows(prev => prev.map(w => w.id === appId ? { ...w, minimized: false } : w));
  }

  function toggleWindow(appId: string) {
    const win = windows().find(w => w.id === appId);
    if (win?.minimized) {
      restoreWindow(appId);
    } else {
      minimizeWindow(appId);
    }
  }

  function renderApp(appId: string) {
    const close = () => closeWindow(appId);
    switch (appId) {
      case APP_IDS.FILES: return <FileBrowser onClose={close} />;
      case APP_IDS.TERMINAL: return <Terminal onClose={close} />;
      case APP_IDS.PROCESSES: return <ProcessManager onClose={close} />;
      case APP_IDS.EDITOR: return <TextEditor onClose={close} />;
      case APP_IDS.IMAGE_VIEWER: return <ImageViewer onClose={close} />;
      default: return null;
    }
  }

  function formatTime(date: Date) {
    return date.toLocaleTimeString("en-US", { hour: "2-digit", minute: "2-digit" });
  }

  return (
    <div class="desktop" onClick={() => setStartMenuOpen(false)}>
      <div class="desktop-icons">
        <For each={Object.entries(APP_META)}>
          {([appId, meta]) => (
            <div class="desktop-icon" onClick={() => openApp(appId)}>
              <span class="icon-img">{meta.icon}</span>
              <span class="icon-label">{meta.title}</span>
            </div>
          )}
        </For>
      </div>

      <For each={windows()}>
        {(win) => {
          const meta = APP_META[win.id as keyof typeof APP_META];
          return (
            <Window
              title={meta.title}
              icon={meta.icon}
              minimized={win.minimized}
              onClose={() => closeWindow(win.id)}
              onMinimize={() => minimizeWindow(win.id)}
              onRestore={() => restoreWindow(win.id)}
            >
              {renderApp(win.id)}
            </Window>
          );
        }}
      </For>

      <div class="taskbar" onClick={(e) => e.stopPropagation()}>
        <button class="start-btn" onClick={() => setStartMenuOpen(!startMenuOpen())}>
          <span class="start-icon">✦</span>
        </button>
        
        <div class="taskbar-apps">
          <For each={windows()}>
            {(win) => {
              const meta = APP_META[win.id as keyof typeof APP_META];
              return (
                <button
                  class={`taskbar-app ${win.minimized ? "minimized" : ""}`}
                  onClick={() => toggleWindow(win.id)}
                  title={win.minimized ? `Restore ${meta.title}` : meta.title}
                >
                  <span>{meta.icon}</span>
                </button>
              );
            }}
          </For>
        </div>
        
        <div class="taskbar-spacer" />
        
        <div class="taskbar-clock">
          <span class="clock-time">{formatTime(clock())}</span>
        </div>
      </div>

      <Show when={startMenuOpen()}>
        <div class="start-menu" onClick={(e) => e.stopPropagation()}>
          <For each={Object.entries(APP_META)}>
            {([appId, meta]) => (
              <div class="start-menu-item" onClick={() => openApp(appId)}>
                <span class="start-menu-icon">{meta.icon}</span>
                <span class="start-menu-label">{meta.title}</span>
              </div>
            )}
          </For>
        </div>
      </Show>
    </div>
  );
}
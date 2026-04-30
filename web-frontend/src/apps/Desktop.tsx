import { createSignal, For, Show, onMount, onCleanup } from "solid-js";
import { 
  Folder, 
  Terminal as TerminalIcon, 
  Activity, 
  FileText, 
  Image as ImageIcon,
  Cpu,
  Settings as SettingsIcon,
  CheckCircle
} from "lucide-solid";
import { Window } from "../components/Window";
import { FileBrowser } from "./FileBrowser";
import { Terminal } from "./Terminal";
import { ProcessManager } from "./ProcessManager";
import { TextEditor } from "./TextEditor";
import { ImageViewer } from "./ImageViewer";
import { Settings } from "./Settings";
import { desktopStore } from "../desktopStore";
import { notificationStore } from "../notificationStore";
import { timeFormat, backgroundImage, loadBackground } from "../settings";

export const APP_IDS = {
  FILES: "files",
  TERMINAL: "terminal",
  PROCESSES: "processes",
  EDITOR: "editor",
  IMAGE_VIEWER: "image_viewer",
  SETTINGS: "settings",
};

export const APP_META: Record<string, { title: string; icon: any }> = {
  [APP_IDS.FILES]: { title: "Files", icon: Folder },
  [APP_IDS.TERMINAL]: { title: "Terminal", icon: TerminalIcon },
  [APP_IDS.PROCESSES]: { title: "Processes", icon: Activity },
  [APP_IDS.EDITOR]: { title: "Editor", icon: FileText },
  [APP_IDS.IMAGE_VIEWER]: { title: "Image Viewer", icon: ImageIcon },
  [APP_IDS.SETTINGS]: { title: "Settings", icon: SettingsIcon },
};

export function Desktop() {
  const [clock, setClock] = createSignal(new Date());

  onMount(() => {
    loadBackground();
    const timer = setInterval(() => setClock(new Date()), 1000);
    onCleanup(() => clearInterval(timer));
  });

  function renderApp(appId: string) {
    const close = () => desktopStore.closeWindow(appId);
    switch (appId) {
      case APP_IDS.FILES: return <FileBrowser onClose={close} />;
      case APP_IDS.TERMINAL: return <Terminal onClose={close} />;
      case APP_IDS.PROCESSES: return <ProcessManager onClose={close} />;
      case APP_IDS.EDITOR: return <TextEditor onClose={close} />;
      case APP_IDS.IMAGE_VIEWER: return <ImageViewer onClose={close} />;
      case APP_IDS.SETTINGS: return <Settings onClose={close} />;
      default: return null;
    }
  }

  function formatTime(date: Date) {
    const is24h = timeFormat() === "24h";
    return date.toLocaleTimeString("en-US", {
      hour: "2-digit",
      minute: "2-digit",
      hour12: !is24h,
    });
  }

  return (
    <div
      class="desktop"
      classList={{ "has-bg": !!backgroundImage() }}
      style={backgroundImage() ? { "--desktop-bg": `url(${backgroundImage()})` } as any : {}}
      onClick={() => desktopStore.setStartMenuOpen(false)}
    >
      <Show when={notificationStore.notification()}>
        <div class="app-notification" style="position: fixed; bottom: 60px; right: 20px; z-index: 9999;">
          <CheckCircle size={14} class="inline-icon" /> {notificationStore.notification()}
        </div>
      </Show>

      <div class="desktop-icons">
        <For each={Object.entries(APP_META)}>
          {([appId, meta]) => {
            const Icon = meta.icon;
            return (
              <div class="desktop-icon" onClick={() => desktopStore.openApp(appId)}>
                <span class="icon-img"><Icon size={42} /></span>
                <span class="icon-label">{meta.title}</span>
              </div>
            );
          }}
        </For>
      </div>

      <For each={desktopStore.windows()}>
        {(win) => {
          const meta = APP_META[win.id];
          const Icon = meta.icon;
          return (
            <Window
              title={meta.title}
              icon={<Icon size={16} />}
              minimized={win.minimized}
              onClose={() => desktopStore.closeWindow(win.id)}
              onMinimize={() => desktopStore.minimizeWindow(win.id)}
              onRestore={() => desktopStore.restoreWindow(win.id)}
            >
              {renderApp(win.id)}
            </Window>
          );
        }}
      </For>

      <div class="taskbar" onClick={(e) => e.stopPropagation()}>
        <button class="start-btn" onClick={() => desktopStore.setStartMenuOpen(!desktopStore.startMenuOpen())}>
          <span class="start-icon"><Cpu size={20} /></span>
        </button>
        
        <div class="taskbar-apps">
          <For each={desktopStore.windows()}>
            {(win) => {
              const meta = APP_META[win.id];
              const Icon = meta.icon;
              return (
                <button
                  class={`taskbar-app ${win.minimized ? "minimized" : ""}`}
                  onClick={() => desktopStore.toggleWindow(win.id)}
                  title={win.minimized ? `Restore ${meta.title}` : meta.title}
                >
                  <Icon size={18} />
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

      <Show when={desktopStore.startMenuOpen()}>
        <div class="start-menu" onClick={(e) => e.stopPropagation()}>
          <For each={Object.entries(APP_META)}>
            {([appId, meta]) => {
              const Icon = meta.icon;
              return (
                <div class="start-menu-item" onClick={() => desktopStore.openApp(appId)}>
                  <Icon size={18} class="start-menu-icon" />
                  <span class="start-menu-label">{meta.title}</span>
                </div>
              );
            }}
          </For>
        </div>
      </Show>
    </div>
  );
}
import { createRoot, createSignal } from "solid-js";

export interface WindowState {
  id: string;
  minimized: boolean;
}

function createDesktopStore() {
  const [windows, setWindows] = createSignal<WindowState[]>([]);
  const [startMenuOpen, setStartMenuOpen] = createSignal(false);

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

  return {
    windows,
    startMenuOpen,
    setStartMenuOpen,
    openApp,
    closeWindow,
    minimizeWindow,
    restoreWindow,
    toggleWindow,
  };
}

export const desktopStore = createRoot(createDesktopStore);

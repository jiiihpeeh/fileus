import { Component, JSX, createSignal, onMount } from "solid-js";
import { getCurrentWindow } from "@tauri-apps/api/window";

interface WindowFrameProps {
  title?: string;
}

interface WindowAppProps {
  children: JSX.Element;
}

const WindowFrame: Component<WindowFrameProps> = (props) => {
  const appWindow = getCurrentWindow();
  const [isMaximized, setIsMaximized] = createSignal(false);

  onMount(async () => {
    setIsMaximized(await appWindow.isMaximized());
  });

  async function minimize() {
    await appWindow.minimize();
  }

  async function toggleMaximize() {
    await appWindow.toggleMaximize();
    setIsMaximized(await appWindow.isMaximized());
  }

  async function close() {
    await appWindow.close();
  }

  function handleMouseDown(e: MouseEvent) {
    const target = e.target as HTMLElement;
    if (target.closest("button")) {
      e.stopPropagation();
      return;
    }
    if (e.buttons === 1) {
      e.detail === 2 ? toggleMaximize() : appWindow.startDragging();
    }
  }

  return (
    <div
      id="titlebar"
      class={`titlebar ${isMaximized() ? "" : "rounded"}`}
      onMouseDown={handleMouseDown}
    >
      <span class="titlebar-title">{props.title || "Fileus"}</span>
      <div class="controls">
        <button id="titlebar-minimize" onClick={minimize} title="minimize">
          <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24">
            <path fill="currentColor" d="M19 13H5v-2h14z" />
          </svg>
        </button>
        <button id="titlebar-maximize" onClick={toggleMaximize} title="maximize">
          {isMaximized() ? (
            <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24">
              <path fill="currentColor" d="M4 4h16v16H4zm2 4v10h12V8z" />
            </svg>
          ) : (
            <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24">
              <path fill="currentColor" d="M4 4h16v16H4z" />
            </svg>
          )}
        </button>
        <button id="titlebar-close" onClick={close} title="close">
          <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24">
            <path fill="currentColor" d="M13.46 12L19 17.54V19h-1.46L12 13.46L6.46 19H5v-1.46L10.54 12L5 6.46V5h1.46L12 10.54L17.54 5H19v1.46z" />
          </svg>
        </button>
      </div>
    </div>
  );
};

const WindowApp: Component<WindowAppProps> = (props) => {
  return (
    <div class="app">
      {props.children}
    </div>
  );
};

interface WindowProps {
  children: JSX.Element;
  title?: string;
}

const Window: Component<WindowProps> = (props) => {
  return (
    <div class="window">
      <WindowFrame title={props.title} />
      <WindowApp>{props.children}</WindowApp>
    </div>
  );
};

export default Window;
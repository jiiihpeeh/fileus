import { createSignal, JSX, Show } from "solid-js";
import { X, Minus, Square, Copy } from "lucide-solid";

interface WindowProps {
  title: string;
  icon: JSX.Element;
  minimized?: boolean;
  onClose: () => void;
  onMinimize?: () => void;
  onRestore?: () => void;
  children: JSX.Element;
}

type ResizeHandle = "nw" | "ne" | "sw" | "se" | null;

export function Window(props: WindowProps) {
  let headerEl: HTMLDivElement | undefined;
  const [pos, setPos] = createSignal({ x: 150, y: 80 });
  const [size, setSize] = createSignal({ w: 600, h: 400 });
  const [maximized, setMaximized] = createSignal(false);
  let dragging = false;
  let resizing: ResizeHandle = null;
  let startX = 0;
  let startY = 0;
  let startW = 0;
  let startH = 0;
  let startPosX = 0;
  let startPosY = 0;

  function saveState() {
    startX = size().w;
    startY = size().h;
    startPosX = pos().x;
    startPosY = pos().y;
  }

  function onHeaderMouseDown(e: MouseEvent) {
    if ((e.target as HTMLElement).closest(".window-controls")) return;
    e.preventDefault();
    dragging = true;
    startX = e.clientX - pos().x;
    startY = e.clientY - pos().y;

    document.addEventListener("mousemove", onMouseMove);
    document.addEventListener("mouseup", onMouseUp);
  }

  function onResizeMouseDown(e: MouseEvent, handle: ResizeHandle) {
    e.preventDefault();
    e.stopPropagation();
    resizing = handle;
    startX = e.clientX;
    startY = e.clientY;
    startW = size().w;
    startH = size().h;
    startPosX = pos().x;
    startPosY = pos().y;

    document.addEventListener("mousemove", onResizeMove);
    document.addEventListener("mouseup", onResizeUp);
  }

  function onMouseMove(e: MouseEvent) {
    if (!dragging) return;
    setPos({ x: e.clientX - startX, y: e.clientY - startY });
  }

  function onResizeMove(e: MouseEvent) {
    if (!resizing) return;
    const dx = e.clientX - startX;
    const dy = e.clientY - startY;

    if (resizing === "se") {
      setSize({ w: Math.max(300, startW + dx), h: Math.max(200, startH + dy) });
    } else if (resizing === "sw") {
      const newW = Math.max(300, startW - dx);
      setPos({ x: startPosX + (startW - newW), y: pos().y });
      setSize({ w: newW, h: Math.max(200, startH + dy) });
    } else if (resizing === "ne") {
      setSize({ w: Math.max(300, startW + dx), h: Math.max(200, startH - dy) });
    } else if (resizing === "nw") {
      const newW = Math.max(300, startW - dx);
      const newH = Math.max(200, startH - dy);
      setPos({ x: startPosX + (startW - newW), y: startPosY + (startH - newH) });
      setSize({ w: newW, h: newH });
    }
  }

  function onMouseUp() {
    dragging = false;
    document.removeEventListener("mousemove", onMouseMove);
    document.removeEventListener("mouseup", onMouseUp);
  }

  function onResizeUp() {
    resizing = null;
    document.removeEventListener("mousemove", onResizeMove);
    document.removeEventListener("mouseup", onResizeUp);
  }

  function toggleMaximize() {
    if (maximized()) {
      setMaximized(false);
    } else {
      saveState();
      setMaximized(true);
    }
  }

  return (
    <Show when={!props.minimized}>
      <div
        class="window"
        classList={{ maximized: maximized() }}
        style={{
          left: maximized() ? "0" : `${pos().x}px`,
          top: maximized() ? "0" : `${pos().y}px`,
          width: maximized() ? "100%" : `${size().w}px`,
          height: maximized() ? "calc(100vh - 48px)" : `${size().h}px`
        }}
      >
        <div
          ref={headerEl}
          class="window-header"
          onMouseDown={onHeaderMouseDown}
        >
          <div style="display: flex; align-items: center; gap: 8px;">
            {props.icon}
            <span class="window-title">{props.title}</span>
          </div>
          <div class="window-controls">
            <button class="win-btn win-min" onClick={props.onMinimize} title="Minimize"><Minus size={14} /></button>
            <button class="win-btn win-max" onClick={toggleMaximize} title={maximized() ? "Restore" : "Maximize"}>
              <Show when={maximized()} fallback={<Square size={12} />}>
                <Copy size={12} />
              </Show>
            </button>
            <button class="win-btn win-close" onClick={props.onClose} title="Close"><X size={14} /></button>
          </div>
        </div>
        <div class="window-body">
          {props.children}
        </div>
        <Show when={!maximized()}>
          <div class="resize-handle nw" onMouseDown={(e) => onResizeMouseDown(e, "nw")} />
          <div class="resize-handle ne" onMouseDown={(e) => onResizeMouseDown(e, "ne")} />
          <div class="resize-handle sw" onMouseDown={(e) => onResizeMouseDown(e, "sw")} />
          <div class="resize-handle se" onMouseDown={(e) => onResizeMouseDown(e, "se")} />
        </Show>
      </div>
    </Show>
  );
}

export default Window;
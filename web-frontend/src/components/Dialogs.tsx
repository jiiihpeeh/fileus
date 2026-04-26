import { createSignal, For, Show, onMount } from "solid-js";
import { apiList, apiGetHome, apiGetDrives, formatSize } from "../api";

type DialogCallback = (path: string | null) => void;

interface FileDialogProps {
  title: string;
  onConfirm: DialogCallback;
  onCancel: () => void;
  saveMode?: boolean;
}

export function FileDialog(props: FileDialogProps) {
  const [currentDir, setCurrentDir] = createSignal("");
  const [files, setFiles] = createSignal<any[]>([]);
  const [selectedFile, setSelectedFile] = createSignal<any>(null);
  const [filename, setFilename] = createSignal("");
  const [drives, setDrives] = createSignal<any[]>([]);

  async function loadDirectory(dir: string) {
    try {
      const items = await apiList(dir);
      setFiles(items.filter((f: any) => !f.name.startsWith(".")));
      setCurrentDir(dir);
      setSelectedFile(null);
    } catch {}
  }

  async function loadDrives() {
    try { setDrives(await apiGetDrives()); } catch {}
  }

  function navigateTo(path: string) {
    loadDirectory(path);
  }

  function navigateUp() {
    const parts = currentDir().split("/").filter(Boolean);
    if (parts.length <= 1) loadDirectory("/");
    else loadDirectory("/" + parts.slice(0, -1).join("/"));
  }

  function handleFileClick(file: any) {
    if (file.is_dir) {
      navigateTo(file.path);
    } else {
      setSelectedFile(file);
      if (!props.saveMode) setFilename(file.name);
    }
  }

  function handleItemDblClick(file: any) {
    if (file.is_dir) navigateTo(file.path);
    else if (!props.saveMode) {
      setSelectedFile(file);
      setFilename(file.name);
    }
  }

  function pathSegments() {
    const parts = currentDir().split("/").filter(Boolean);
    const segs = [{ name: "root", path: "/" }];
    let p = "";
    for (const part of parts) {
      p += "/" + part;
      segs.push({ name: part, path: p });
    }
    return segs;
  }

  function handleConfirm() {
    if (props.saveMode) {
      const name = filename().trim();
      if (!name) return;
      const fullPath = currentDir() === "/" ? `/${name}` : `${currentDir()}/${name}`;
      props.onConfirm(fullPath);
    } else {
      if (selectedFile() && !selectedFile()?.is_dir) {
        props.onConfirm(selectedFile()!.path);
      } else if (filename().trim()) {
        const fullPath = currentDir() === "/" ? `/${filename().trim()}` : `${currentDir()}/${filename().trim()}`;
        props.onConfirm(fullPath);
      }
    }
  }

  onMount(async () => {
    await loadDrives();
    const home = await apiGetHome();
    await loadDirectory(home.path);
  });

  return (
    <div class="dialog-overlay" onClick={(e) => e.target === e.currentTarget && props.onCancel()}>
      <div class="dialog">
        <div class="dialog-header">
          <h3>{props.title}</h3>
          <button class="btn-sm" onClick={props.onCancel}>✕</button>
        </div>
        <div class="dialog-body">
          <div class="path-bar">
            <button class="btn-sm" onClick={() => loadDirectory("/")}>🏠</button>
            <button class="btn-sm" onClick={navigateUp}>⬆</button>
            <For each={pathSegments()}>
              {(seg, i) => (
                <>
                  <Show when={i() > 0}><span class="path-sep">/</span></Show>
                  <span class="path-segment" onClick={() => navigateTo(seg.path)}>{seg.name}</span>
                </>
              )}
            </For>
          </div>
          <div class="file-dialog-list">
            <For each={files()}>
              {(file) => (
                <div
                  class={`file-dialog-item ${selectedFile()?.path === file.path ? "selected" : ""}`}
                  onClick={() => handleFileClick(file)}
                  onDblClick={() => handleItemDblClick(file)}
                >
                  <span class="file-icon">{file.is_dir ? "📁" : "📄"}</span>
                  <div class="file-info">
                    <div class="file-name">{file.name}</div>
                    <div class="file-meta">{file.is_dir ? "Folder" : formatSize(file.size)}</div>
                  </div>
                </div>
              )}
            </For>
          </div>
          <Show when={props.saveMode || !selectedFile()}>
            <input
              class="dialog-input"
              style={{ "margin-top": "8px" }}
              placeholder={props.saveMode ? "filename.txt" : "or enter path manually"}
              value={filename()}
              onInput={(e) => setFilename((e.target as HTMLInputElement).value)}
              onKeyPress={(e) => e.key === "Enter" && handleConfirm()}
            />
          </Show>
        </div>
        <div class="dialog-footer">
          <button class="btn-sm" onClick={props.onCancel}>Cancel</button>
          <button class="btn-sm btn-primary" onClick={handleConfirm} disabled={props.saveMode ? !filename().trim() : !selectedFile()}>
            {props.saveMode ? "Save" : "Open"}
          </button>
        </div>
      </div>
    </div>
  );
}

interface SaveDialogProps {
  initialPath?: string;
  onConfirm: (path: string, content: string) => void;
  onCancel: () => void;
}

export function SaveDialog(props: SaveDialogProps) {
  const [path, setPath] = createSignal(props.initialPath || "");
  const [content, setContent] = createSignal("");
  const [showFileDialog, setShowFileDialog] = createSignal(false);

  function handleSave() {
    const p = path().trim();
    if (!p) return;
    props.onConfirm(p, content());
  }

  return (
    <div class="dialog-overlay" onClick={(e) => e.target === e.currentTarget && props.onCancel()}>
      <div class="dialog">
        <div class="dialog-header">
          <h3>Save File</h3>
          <button class="btn-sm" onClick={props.onCancel}>✕</button>
        </div>
        <div class="dialog-body">
          <div style={{ display: "flex", gap: "8px", "margin-bottom": "8px" }}>
            <input
              class="dialog-input"
              style={{ flex: 1 }}
              placeholder="/path/to/file.txt"
              value={path()}
              onInput={(e) => setPath((e.target as HTMLInputElement).value)}
            />
            <button class="btn-sm" onClick={() => setShowFileDialog(true)}>📂</button>
          </div>
          <Show when={showFileDialog()}>
            <FileDialog title="Save As" saveMode={true} onConfirm={(p) => { setPath(p); setShowFileDialog(false); }} onCancel={() => setShowFileDialog(false)} />
          </Show>
          <textarea
            class="dialog-input"
            style={{ height: "150px", resize: "vertical", width: "100%", "margin-top": "8px" }}
            placeholder="File content..."
            value={content()}
            onInput={(e) => setContent((e.target as HTMLTextAreaElement).value)}
          />
        </div>
        <div class="dialog-footer">
          <button class="btn-sm" onClick={props.onCancel}>Cancel</button>
          <button class="btn-sm btn-primary" onClick={handleSave} disabled={!path().trim()}>Save</button>
        </div>
      </div>
    </div>
  );
}
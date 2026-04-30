import { createSignal, For, Show, onMount } from "solid-js";
import { X, Home, ArrowUp, Folder, File, Save, FolderOpen, Eye, EyeOff } from "lucide-solid";
import { apiList, apiGetHome, apiGetDrives, formatSize } from "../api";
import "./Dialogs.css";

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
  const [showHidden, setShowHidden] = createSignal(false);

  async function loadDirectory(dir: string) {
    try {
      const resp = await apiList(dir);
      let items = resp.items || [];
      if (!showHidden()) items = items.filter((f: any) => !f.name.startsWith("."));
      setFiles(items);
      setCurrentDir(dir);
      setSelectedFile(null);
    } catch {}
  }

  async function loadDrives() {
    // try { setDrives(await apiGetDrives()); } catch {}
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
      handleConfirm();
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
          <button class="win-btn win-close" onClick={props.onCancel}><X size={14} /></button>
        </div>
        <div class="dialog-body">
          <div class="proc-toolbar" style="margin-bottom: 12px; border-radius: 8px; border: 1px solid var(--border);">
            <div style="display: flex; align-items: center; gap: 4px; flex: 1; overflow: hidden;">
              <button class="btn-sm" onClick={() => loadDirectory("/")}><Home size={14} /></button>
              <button class="btn-sm" onClick={navigateUp}><ArrowUp size={14} /></button>
              <div style="display: flex; gap: 2px; align-items: center; overflow: hidden; font-size: 12px;">
                <For each={pathSegments()}>
                  {(seg, i) => (
                    <>
                      <Show when={i() > 0}><span style="opacity: 0.5;">/</span></Show>
                      <span class="path-segment" style="cursor: pointer; white-space: nowrap;" onClick={() => navigateTo(seg.path)}>{seg.name}</span>
                    </>
                  )}
                </For>
              </div>
            </div>
            <button
              class="btn-sm"
              style="flex-shrink: 0;"
              onClick={() => { setShowHidden(!showHidden()); loadDirectory(currentDir()); }}
              title={showHidden() ? "Hide Hidden Files" : "Show Hidden Files"}
            >
              <Show when={showHidden()} fallback={<EyeOff size={14} />}>
                <Eye size={14} />
              </Show>
            </button>
          </div>
          <div class="file-dialog-list" style="height: 250px; overflow-y: auto; background: var(--bg-primary); border: 1px solid var(--border); border-radius: 8px;">
            <For each={files()}>
              {(file) => (
                <div
                  class={`file-dialog-item ${selectedFile()?.path === file.path ? "selected" : ""}`}
                  style={{ 
                    display: "flex", 
                    "align-items": "center", 
                    gap: "10px", 
                    padding: "8px 12px", 
                    cursor: "pointer",
                    background: selectedFile()?.path === file.path ? "var(--accent)" : "transparent",
                    color: selectedFile()?.path === file.path ? "white" : "inherit"
                  }}
                  onClick={() => handleFileClick(file)}
                  onDblClick={() => handleItemDblClick(file)}
                >
                  <span style={{ color: selectedFile()?.path === file.path ? "white" : "var(--accent)" }}>
                    {file.is_dir ? <Folder size={16} /> : <File size={16} />}
                  </span>
                  <div style="flex: 1; overflow: hidden;">
                    <div style={{ "font-size": "13px", "font-weight": "500", overflow: "hidden", "text-overflow": "ellipsis", "white-space": "nowrap" }}>{file.name}</div>
                    <div style={{ "font-size": "11px", opacity: 0.7 }}>{file.is_dir ? "Folder" : formatSize(file.size)}</div>
                  </div>
                </div>
              )}
            </For>
          </div>
          <Show when={props.saveMode || !selectedFile()}>
            <div class="dialog-input-group" style="margin-top: 12px;">
              <label>{props.saveMode ? "Filename" : "Selected Path"}</label>
              <input
                class="input"
                placeholder={props.saveMode ? "filename.txt" : "or enter path manually"}
                value={filename()}
                onInput={(e) => setFilename((e.target as HTMLInputElement).value)}
                onKeyPress={(e) => e.key === "Enter" && handleConfirm()}
              />
            </div>
          </Show>
        </div>
        <div class="dialog-footer">
          <button class="btn-sm" onClick={props.onCancel}>Cancel</button>
          <button class="btn-sm btn-primary" onClick={handleConfirm} disabled={props.saveMode ? !filename().trim() : !selectedFile() && !filename().trim()}>
            <Show when={props.saveMode} fallback={<FolderOpen size={14} />}>
              <Save size={14} />
            </Show>
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
      <div class="dialog" style="min-width: 500px;">
        <div class="dialog-header">
          <h3>Save File</h3>
          <button class="win-btn win-close" onClick={props.onCancel}><X size={14} /></button>
        </div>
        <div class="dialog-body">
          <div class="dialog-input-group" style="margin-bottom: 16px;">
            <label>Save Path</label>
            <div style={{ display: "flex", gap: "8px" }}>
              <input
                class="input"
                style={{ flex: 1 }}
                placeholder="/path/to/file.txt"
                value={path()}
                onInput={(e) => setPath((e.target as HTMLInputElement).value)}
              />
              <button class="btn-sm" onClick={() => setShowFileDialog(true)}><FolderOpen size={14} /></button>
            </div>
          </div>
          
          <div class="dialog-input-group">
            <label>File Content</label>
            <textarea
              class="input"
              style={{ height: "200px", resize: "vertical", width: "100%", "font-family": "var(--mono)" }}
              placeholder="File content..."
              value={content()}
              onInput={(e) => setContent((e.target as HTMLTextAreaElement).value)}
            />
          </div>

          <Show when={showFileDialog()}>
            <FileDialog title="Save As" saveMode={true} onConfirm={(p) => { if(p) setPath(p); setShowFileDialog(false); }} onCancel={() => setShowFileDialog(false)} />
          </Show>
        </div>
        <div class="dialog-footer">
          <button class="btn-sm" onClick={props.onCancel}>Cancel</button>
          <button class="btn-sm btn-primary" onClick={handleSave} disabled={!path().trim()}>
            <Save size={14} /> Save
          </button>
        </div>
      </div>
    </div>
  );
}
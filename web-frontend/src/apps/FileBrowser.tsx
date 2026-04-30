import { createSignal, For, Show, onMount } from "solid-js";
import { apiList, apiCreateDir, apiDelete, apiRename, apiCopy, apiMove, apiGetHome, apiGetDrives, apiDownload, formatSize, formatDate } from "../api";
import "./FileBrowser.css";

interface ContextMenuItem {
  label: string;
  icon?: string;
  action: () => void;
  danger?: boolean;
  disabled?: boolean;
}

interface FileBrowserProps {
  onClose: () => void;
  onOpenImage?: (path: string) => void;
}

export function FileBrowser(_props: FileBrowserProps) {
  const [currentDir, setCurrentDir] = createSignal("");
  const [files, setFiles] = createSignal<any[]>([]);
  const [selectedFile, setSelectedFile] = createSignal<any>(null);
  const [drives, setDrives] = createSignal<any[]>([]);
  const [showNewFolder, setShowNewFolder] = createSignal(false);
  const [newFolderName, setNewFolderName] = createSignal("");
  const [showCopyDest, setShowCopyDest] = createSignal(false);
  const [copyDestPath, setCopyDestPath] = createSignal("");
  const [notification, setNotification] = createSignal("");
  const [showRename, setShowRename] = createSignal(false);
  const [renameName, setRenameName] = createSignal("");
  const [sortBy, setSortBy] = createSignal<"name" | "size" | "modified" | "owner" | "permissions">("name");
  const [sortAsc, setSortAsc] = createSignal(true);
  const [showHidden, setShowHidden] = createSignal(false);
  const [filterExt, setFilterExt] = createSignal("");
  const [contextMenu, setContextMenu] = createSignal<{ x: number; y: number; file: any } | null>(null);
  const [splitView, setSplitView] = createSignal(false);
  const [splitDir, setSplitDir] = createSignal("");
  const [splitFiles, setSplitFiles] = createSignal<any[]>([]);
  const [dropTarget, setDropTarget] = createSignal<"main" | "split" | null>(null);
  const [activePane, setActivePane] = createSignal<"main" | "split">("main");

  async function loadDrives() {
    try { setDrives(await apiGetDrives()); } catch {}
  }

  async function loadDirectory(dir?: string) {
    const targetDir = dir || currentDir() || "/";
    try {
      const resp = await apiList(targetDir);
      setFiles(Array.isArray(resp.items) ? resp.items : []);
      setCurrentDir(targetDir);
      setSelectedFile(null);
    } catch (err) { showNotification(`Error: ${err}`); }
  }

  async function loadSplitDirectory(dir?: string) {
    const targetDir = dir || splitDir() || "/";
    try {
      const resp = await apiList(targetDir);
      setSplitFiles(Array.isArray(resp.items) ? resp.items : []);
      setSplitDir(targetDir);
    } catch (err) { showNotification(`Error: ${err}`); }
  }

  function toggleSplitView() {
    if (splitView()) {
      setSplitView(false);
    } else {
      setSplitView(true);
      setSplitDir(currentDir() || "/");
      loadSplitDirectory(currentDir());
    }
  }

  function getActiveDir() {
    return activePane() === "split" ? splitDir() : currentDir();
  }

  async function navigateActive(path: string) {
    if (activePane() === "split") {
      await loadSplitDirectory(path);
    } else {
      await loadDirectory(path);
    }
  }

  async function navigateUpActive() {
    const dir = getActiveDir();
    const parts = dir.split("/").filter(Boolean);
    if (parts.length <= 1) await navigateActive("/");
    else await navigateActive("/" + parts.slice(0, -1).join("/"));
  }

  async function goHomeActive() {
    try {
      const home = await apiGetHome();
      await navigateActive(home.path);
    } catch {
      await navigateActive("/");
    }
  }

  function isHidden(file: any) { return file.name.startsWith("."); }
  function matchesFilter(file: any) {
    const ext = filterExt().toLowerCase().trim();
    if (!ext) return true;
    if (ext.startsWith(".")) return file.name.toLowerCase().endsWith(ext);
    return file.name.toLowerCase().includes(ext);
  }
  function isImage(file: any) {
    if (file.is_dir) return false;
    const ext = file.name.split(".").pop()?.toLowerCase() || "";
    return ["jpg", "jpeg", "png", "gif", "webp", "svg", "bmp", "avif"].includes(ext);
  }

  function sortedFiltered(fileList: any[]) {
    let f = [...fileList];
    if (!showHidden()) f = f.filter(f => !isHidden(f));
    const ext = filterExt().trim();
    if (ext) f = f.filter(f => matchesFilter(f));
    const by = sortBy();
    const asc = sortAsc();
    f.sort((a, b) => {
      if (a.is_dir !== b.is_dir) return a.is_dir ? -1 : 1;
      let cmp = 0;
      if (by === "name") cmp = a.name.localeCompare(b.name);
      else if (by === "size") cmp = a.size - b.size;
      else if (by === "modified") cmp = (a.modified || 0) - (b.modified || 0);
      else if (by === "owner") cmp = (a.owner || "").localeCompare(b.owner || "");
      else if (by === "permissions") cmp = (a.permissions || "").localeCompare(b.permissions || "");
      else cmp = a.name.localeCompare(b.name);
      return asc ? cmp : -cmp;
    });
    return f;
  }

  function toggleSort(col: any) {
    if (sortBy() === col) setSortAsc(!sortAsc());
    else { setSortBy(col); setSortAsc(true); }
  }

  async function navigateTo(path: string) { await loadDirectory(path); }
  async function navigateSplitTo(path: string) { await loadSplitDirectory(path); }
  async function goHome() {
    try { const home = await apiGetHome(); await loadDirectory(home.path); }
    catch { await loadDirectory("/"); }
  }

  async function createNewFolder() {
    const name = newFolderName().trim();
    if (!name) return;
    const path = currentDir() === "/" ? `/${name}` : `${currentDir()}/${name}`;
    try {
      await apiCreateDir(path);
      setNewFolderName("");
      setShowNewFolder(false);
      await loadDirectory();
      showNotification("Folder created");
    } catch (err) { showNotification(`Error: ${err}`); }
  }

  async function deleteFile() {
    const file = selectedFile();
    if (!file || !confirm(`Delete "${file.name}"?`)) return;
    try {
      await apiDelete(file.path);
      setSelectedFile(null);
      await loadDirectory();
      showNotification("Deleted");
    } catch (err) { showNotification(`Error: ${err}`); }
  }

  async function renameFile() {
    const file = selectedFile();
    if (!file) return;
    const newName = renameName().trim();
    if (!newName) return;
    const parent = file.path.substring(0, file.path.lastIndexOf("/"));
    const newPath = parent === "" ? `/${newName}` : `${parent}/${newName}`;
    try {
      await apiRename(file.path, newPath);
      setShowRename(false);
      await loadDirectory();
      showNotification("Renamed");
    } catch (err) { showNotification(`Error: ${err}`); }
  }

  async function copyFile(toMove = false) {
    const file = selectedFile();
    if (!file) return;
    const dest = copyDestPath().trim();
    if (!dest) return;
    const destPath = dest.endsWith("/") ? dest + file.name : dest + "/" + file.name;
    try {
      if (toMove) await apiMove(file.path, destPath);
      else await apiCopy(file.path, destPath);
      setShowCopyDest(false);
      setCopyDestPath("");
      if (toMove) await loadDirectory();
      showNotification(toMove ? "Moved" : "Copied");
    } catch (err) { showNotification(`Error: ${err}`); }
  }

  async function handleDrop(e: DragEvent, target: "main" | "split") {
    const filePath = e.dataTransfer?.getData("text/plain");
    if (!filePath) return;
    
    const targetDir = target === "main" ? currentDir() : splitDir();
    if (!targetDir) return;
    
    const fileName = filePath.split("/").pop() || "";
    const destPath = targetDir === "/" ? `/${fileName}` : `${targetDir}/${fileName}`;
    
    try {
      await apiMove(filePath, destPath);
      setDropTarget(null);
      await loadDirectory();
      if (splitView()) await loadSplitDirectory();
      showNotification("Moved");
    } catch (err) { showNotification(`Error: ${err}`); }
  }

  function showNotification(msg: string) {
    setNotification(msg);
    setTimeout(() => setNotification(""), 2500);
  }

  function handleContextMenu(e: MouseEvent, file: any) {
    e.preventDefault();
    setSelectedFile(file);
    setContextMenu({ x: e.clientX, y: e.clientY, file });
  }

  function closeContextMenu() {
    setContextMenu(null);
  }

  function handleContextAction(action: () => void) {
    action();
    closeContextMenu();
  }

  function contextMenuItems(file: any): ContextMenuItem[] {
    const items: ContextMenuItem[] = [];
    if (file.is_dir) {
      items.push({ label: "Open", icon: "📂", action: () => navigateTo(file.path) });
      items.push({ label: "Download as ZIP", icon: "📦", action: () => apiDownload(file.path) });
      items.push({ label: "", action: () => {} });
    } else {
      items.push({ label: "Open", icon: "📄", action: () => {} });
      if (isImage(file)) {
        items.push({ label: "Open as Image", icon: "🖼️", action: () => _props.onOpenImage?.(file.path) });
      }
      items.push({ label: "Download", icon: "📥", action: () => apiDownload(file.path) });
      items.push({ label: "", action: () => {} });
    }
    items.push(
      { label: "Rename", icon: "✏️", action: () => { setShowRename(true); setRenameName(file.name); } },
      { label: "Copy", icon: "📋", action: () => { setShowCopyDest(true); setCopyDestPath(currentDir()); setSelectedFile(file); } },
      { label: "Move", icon: "📦", action: () => { setShowCopyDest(true); setCopyDestPath(currentDir()); setSelectedFile(file); } },
      { label: "", action: () => {} },
      { label: "Delete", icon: "🗑️", action: () => { setSelectedFile(file); deleteFile(); }, danger: true }
    );
    return items;
  }

  onMount(async () => { await loadDrives(); await goHome(); });

  const FileGridHeader = () => (
    <div class="files-grid-header">
      <div onClick={() => toggleSort("name")}></div>
      <div onClick={() => toggleSort("name")}>Name {sortBy() === "name" ? (sortAsc() ? "↑" : "↓") : ""}</div>
      <div onClick={() => toggleSort("size")}>Size {sortBy() === "size" ? (sortAsc() ? "↑" : "↓") : ""}</div>
      <div onClick={() => toggleSort("owner")}>Owner {sortBy() === "owner" ? (sortAsc() ? "↑" : "↓") : ""}</div>
      <div onClick={() => toggleSort("permissions")}>Perms {sortBy() === "permissions" ? (sortAsc() ? "↑" : "↓") : ""}</div>
      <div onClick={() => toggleSort("modified")}>Date {sortBy() === "modified" ? (sortAsc() ? "↑" : "↓") : ""}</div>
    </div>
  );

  const FileRow = (props: { file: any, pane: "main" | "split" }) => (
    <div
      class={`files-grid-row ${selectedFile()?.path === props.file.path ? "selected" : ""} ${props.file.is_dir ? "folder" : "file"} ${isHidden(props.file) ? "hidden" : ""}`}
      draggable={true}
      onDragStart={(e) => { e.dataTransfer?.setData("text/plain", props.file.path); }}
      onClick={(e) => { e.stopPropagation(); setSelectedFile(props.file); setActivePane(props.pane); }}
      onDblClick={() => props.file.is_dir ? (props.pane === "main" ? navigateTo(props.file.path) : navigateSplitTo(props.file.path)) : isImage(props.file) ? _props.onOpenImage?.(props.file.path) : null}
      onContextMenu={(e) => handleContextMenu(e, props.file)}
    >
      <div>{props.file.is_dir ? "📁" : isImage(props.file) ? "🖼️" : "📄"}</div>
      <div title={props.file.name}>{props.file.name}</div>
      <div class="file-meta-info">{props.file.is_dir ? "-" : formatSize(props.file.size)}</div>
      <div class="file-meta-info">{props.file.owner || "-"}</div>
      <div class="file-meta-info">{props.file.permissions || "-"}</div>
      <div class="file-meta-info">{formatDate(props.file.modified)}</div>
    </div>
  );

  return (
    <div class="app-files" onClick={closeContextMenu}>
      <Show when={notification()}>
        <div class="app-notification">{notification()}</div>
      </Show>

      <Show when={contextMenu()}>
        <div
          class="context-menu"
          style={{ left: `${contextMenu()!.x}px`, top: `${contextMenu()!.y}px` }}
          onClick={(e) => e.stopPropagation()}
        >
          <For each={contextMenuItems(contextMenu()!.file)}>
            {(item) => {
              if (item.label === "") return <div class="context-menu-sep" />;
              return (
                <div
                  class={`context-menu-item ${item.danger ? "danger" : ""} ${item.disabled ? "disabled" : ""}`}
                  onClick={() => !item.disabled && handleContextAction(item.action)}
                >
                  <Show when={item.icon}><span>{item.icon}</span></Show>
                  <span>{item.label}</span>
                </div>
              );
            }}
          </For>
        </div>
      </Show>

      <div class="files-toolbar">
        <button class="btn-sm" onClick={goHomeActive} title="Home">🏠</button>
        <button class="btn-sm" onClick={navigateUpActive} title="Up">⬆</button>
        <button class="btn-sm" onClick={() => navigateActive(getActiveDir())} title="Refresh">⟳</button>
        <button class={`btn-sm ${splitView() ? "active" : ""}`} onClick={toggleSplitView} title="Split View">⫨</button>
        <span class="path-display">{getActiveDir() || "/"}</span>
        <button class="btn-sm" onClick={() => setShowNewFolder(true)} title="New Folder">+ Folder</button>
        <span class="toolbar-sep">|</span>
        <label class="btn-sm toggle-btn">
          <input type="checkbox" checked={showHidden()} onChange={(e) => setShowHidden((e.target as HTMLInputElement).checked)} />
          Hidden
        </label>
        <input
          class="input filter-input"
          placeholder="Filter..."
          value={filterExt()}
          onInput={(e) => setFilterExt((e.target as HTMLInputElement).value)}
        />
      </div>

      <div class="files-body">
        <div class="files-sidebar">
          <div class="sidebar-section">
            <h4>Places</h4>
            <button class="sidebar-item" onClick={goHome}>📁 Home</button>
            <For each={drives()}>
              {(d) => <button class="sidebar-item" onClick={() => navigateTo(d.path)}>💾 {d.name}</button>}
            </For>
            <button class="sidebar-item" onClick={() => navigateTo("/tmp")}>📄 Temp</button>
          </div>
        </div>

        <div class="files-main">
          <Show when={showNewFolder()}>
            <div class="modal-small">
              <input class="input" placeholder="Folder name" value={newFolderName()}
                onInput={(e) => setNewFolderName((e.target as HTMLInputElement).value)}
                onKeyPress={(e) => e.key === "Enter" && createNewFolder()} autofocus />
              <button class="btn-sm" onClick={() => setShowNewFolder(false)}>Cancel</button>
              <button class="btn-sm btn-primary" onClick={createNewFolder}>Create</button>
            </div>
          </Show>

          <Show when={showRename()}>
            <div class="modal-small">
              <input class="input" placeholder="New name" value={renameName()}
                onInput={(e) => setRenameName((e.target as HTMLInputElement).value)}
                onKeyPress={(e) => e.key === "Enter" && renameFile()} autofocus />
              <button class="btn-sm" onClick={() => setShowRename(false)}>Cancel</button>
              <button class="btn-sm btn-primary" onClick={renameFile}>Rename</button>
            </div>
          </Show>

          <Show when={showCopyDest()}>
            <div class="modal-small">
              <input class="input" placeholder="/destination/path" value={copyDestPath()}
                onInput={(e) => setCopyDestPath((e.target as HTMLInputElement).value)} autofocus />
              <button class="btn-sm" onClick={() => copyFile(false)}>Copy</button>
              <button class="btn-sm" onClick={() => copyFile(true)}>Move</button>
              <button class="btn-sm" onClick={() => setShowCopyDest(false)}>Cancel</button>
            </div>
          </Show>

          <div class="files-list" classList={{ "is-split": splitView() }}>
            <div 
              class="files-list-pane" 
              classList={{ "drop-target": dropTarget() === "main", "active-pane": activePane() === "main" }}
              onClick={() => setActivePane("main")}
              onDragOver={(e) => { e.preventDefault(); setDropTarget("main"); }}
              onDragLeave={() => setDropTarget(null)}
              onDrop={(e) => { e.preventDefault(); handleDrop(e, "main"); }}
            >
              <div class="files-scroll-area">
                <div class="files-grid">
                  <FileGridHeader />
                  <For each={sortedFiltered(files())}>
                    {(file) => <FileRow file={file} pane="main" />}
                  </For>
                </div>
              </div>
            </div>

            <Show when={splitView()}>
              <div 
                class="files-list-pane" 
                classList={{ "drop-target": dropTarget() === "split", "active-pane": activePane() === "split" }}
                onClick={() => setActivePane("split")}
                onDragOver={(e) => { e.preventDefault(); setDropTarget("split"); }}
                onDragLeave={() => setDropTarget(null)}
                onDrop={(e) => { e.preventDefault(); handleDrop(e, "split"); }}
              >
                <div class="files-scroll-area">
                  <div class="files-grid">
                    <FileGridHeader />
                    <For each={sortedFiltered(splitFiles())}>
                      {(file) => <FileRow file={file} pane="split" />}
                    </For>
                  </div>
                </div>
              </div>
            </Show>
          </div>

          <Show when={selectedFile()}>
            <div class="files-details">
              <div class="details-info">
                <strong>Name:</strong> <span>{selectedFile()?.name}</span>
                <strong>Path:</strong> <span>{selectedFile()?.path}</span>
                <strong>Size:</strong> <span>{selectedFile()?.is_dir ? "-" : formatSize(selectedFile()?.size)}</span>
                <strong>Modified:</strong> <span>{formatDate(selectedFile()?.modified)}</span>
              </div>
              <div class="details-actions">
                <button class="btn-sm" onClick={() => { setShowRename(true); setRenameName(selectedFile()!.name); }}>Rename</button>
                <button class="btn-sm" onClick={() => setShowCopyDest(true)}>Copy/Move</button>
                <button class="btn-sm btn-danger" onClick={deleteFile}>Delete</button>
                <button class="btn-sm" onClick={() => setSelectedFile(null)}>Close</button>
              </div>
            </div>
          </Show>
        </div>
      </div>
    </div>
  );
}

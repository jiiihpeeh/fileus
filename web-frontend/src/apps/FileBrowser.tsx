import { createSignal, createMemo, For, Show, onMount } from "solid-js";
import { 
  Home, 
  ArrowUp, 
  RefreshCw, 
  Columns2, 
  FolderPlus, 
  Folder, 
  File, 
  Image, 
  Download, 
  Pencil, 
  Copy, 
  ExternalLink,
  Trash2,
  HardDrive,
  Clock,
  User,
  ShieldCheck,
  Eye,
  EyeOff,
  Plus,
  X
} from "lucide-solid";
import { apiList, apiCreateDir, apiDelete, apiRename, apiCopy, apiMove, apiGetHome, apiGetDrives, apiDownload, formatSize, formatDate } from "../api";
import "./FileBrowser.css";

interface ContextMenuItem {
  label: string;
  icon?: any;
  action: () => void;
  danger?: boolean;
  disabled?: boolean;
}

interface FileBrowserTab {
  id: string;
  currentDir: string;
  files: any[];
  selectedFile: any | null;
  splitView: boolean;
  splitDir: string;
  splitFiles: any[];
  activePane: "main" | "split";
}

interface FileBrowserProps {
  onClose: () => void;
  onOpenImage?: (path: string) => void;
}

let _tabId = 0;
function newTabId() { return `tab_${++_tabId}`; }

function createTab(id: string, dir = ""): FileBrowserTab {
  return { id, currentDir: dir, files: [], selectedFile: null, splitView: false, splitDir: "", splitFiles: [], activePane: "main" };
}

export function FileBrowser(_props: FileBrowserProps) {
  const firstTab = createTab(newTabId());
  const [tabs, setTabs] = createSignal<FileBrowserTab[]>([firstTab]);
  const [activeTabId, setActiveTabId] = createSignal(firstTab.id);
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
  const [dropTarget, setDropTarget] = createSignal<"main" | "split" | null>(null);

  const activeTab = createMemo(() => tabs().find(t => t.id === activeTabId())!);

  function updateActiveTab(updates: Partial<FileBrowserTab>) {
    setTabs(prev => prev.map(t => t.id === activeTabId() ? { ...t, ...updates } : t));
  }

  function switchTab(id: string) {
    const tab = tabs().find(t => t.id === id);
    if (tab) setActiveTabId(id);
  }

  async function addTab(dir?: string) {
    const tab = createTab(newTabId(), dir || activeTab()?.currentDir || "/");
    setTabs(prev => [...prev, tab]);
    setActiveTabId(tab.id);
    await loadTabDirectory(tab.id, dir || tab.currentDir);
  }

  function closeTab(id: string) {
    if (tabs().length <= 1) return;
    const idx = tabs().findIndex(t => t.id === id);
    setTabs(prev => prev.filter(t => t.id !== id));
    if (activeTabId() === id) {
      const remaining = tabs().filter(t => t.id !== id);
      const newIdx = Math.min(idx, remaining.length - 1);
      setActiveTabId(remaining[newIdx].id);
    }
  }

  async function loadTabDirectory(tabId: string, dir: string) {
    try {
      const resp = await apiList(dir);
      setTabs(prev => prev.map(t => t.id === tabId ? { ...t, currentDir: dir, files: Array.isArray(resp.items) ? resp.items : [], selectedFile: null } : t));
    } catch (err) { showNotification(`Error: ${err}`); }
  }

  async function loadTabSplitDirectory(tabId: string, dir: string) {
    try {
      const resp = await apiList(dir);
      setTabs(prev => prev.map(t => t.id === tabId ? { ...t, splitDir: dir, splitFiles: Array.isArray(resp.items) ? resp.items : [] } : t));
    } catch (err) { showNotification(`Error: ${err}`); }
  }

  async function loadDrives() {
    try { setDrives(await apiGetDrives()); } catch {}
  }

  async function loadSplitDirectory(dir?: string) {
    const tab = activeTab();
    if (!tab) return;
    await loadTabSplitDirectory(tab.id, dir || tab.splitDir || "/");
  }

  function toggleSplitView() {
    const tab = activeTab();
    if (!tab) return;
    if (tab.splitView) {
      updateActiveTab({ splitView: false });
    } else {
      updateActiveTab({ splitView: true, splitDir: tab.currentDir || "/" });
      loadSplitDirectory(tab.currentDir);
    }
  }

  function getActiveDir() {
    const tab = activeTab();
    if (!tab) return "/";
    return tab.activePane === "split" ? tab.splitDir : tab.currentDir;
  }

  async function navigateActive(path: string) {
    const tab = activeTab();
    if (!tab) return;
    if (tab.activePane === "split") {
      await loadTabSplitDirectory(tab.id, path);
    } else {
      await loadTabDirectory(tab.id, path);
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

  async function navigateTo(path: string) {
    const tab = activeTab();
    if (tab) await loadTabDirectory(tab.id, path);
  }
  async function navigateSplitTo(path: string) {
    const tab = activeTab();
    if (tab) await loadTabSplitDirectory(tab.id, path);
  }
  async function goHome() {
    const tab = activeTab();
    if (!tab) return;
    try { const home = await apiGetHome(); await loadTabDirectory(tab.id, home.path); }
    catch { await loadTabDirectory(tab.id, "/"); }
  }

  function tabTitle(tab: FileBrowserTab) {
    if (!tab.currentDir || tab.currentDir === "/") return "/";
    const parts = tab.currentDir.split("/").filter(Boolean);
    return parts[parts.length - 1] || "/";
  }

  async function createNewFolder() {
    const name = newFolderName().trim();
    if (!name) return;
    const tab = activeTab();
    if (!tab) return;
    const path = tab.currentDir === "/" ? `/${name}` : `${tab.currentDir}/${name}`;
    try {
      await apiCreateDir(path);
      setNewFolderName("");
      setShowNewFolder(false);
      await loadTabDirectory(tab.id, tab.currentDir);
      showNotification("Folder created");
    } catch (err) { showNotification(`Error: ${err}`); }
  }

  async function deleteFile() {
    const tab = activeTab();
    if (!tab) return;
    const file = tab.selectedFile;
    if (!file || !confirm(`Delete "${file.name}"?`)) return;
    try {
      await apiDelete(file.path);
      updateActiveTab({ selectedFile: null });
      await loadTabDirectory(tab.id, tab.currentDir);
      showNotification("Deleted");
    } catch (err) { showNotification(`Error: ${err}`); }
  }

  async function renameFile() {
    const tab = activeTab();
    if (!tab) return;
    const file = tab.selectedFile;
    if (!file) return;
    const newName = renameName().trim();
    if (!newName) return;
    const parent = file.path.substring(0, file.path.lastIndexOf("/"));
    const newPath = parent === "" ? `/${newName}` : `${parent}/${newName}`;
    try {
      await apiRename(file.path, newPath);
      setShowRename(false);
      await loadTabDirectory(tab.id, tab.currentDir);
      showNotification("Renamed");
    } catch (err) { showNotification(`Error: ${err}`); }
  }

  async function copyFile(toMove = false) {
    const tab = activeTab();
    if (!tab) return;
    const file = tab.selectedFile;
    if (!file) return;
    const dest = copyDestPath().trim();
    if (!dest) return;
    const destPath = dest.endsWith("/") ? dest + file.name : dest + "/" + file.name;
    try {
      if (toMove) await apiMove(file.path, destPath);
      else await apiCopy(file.path, destPath);
      setShowCopyDest(false);
      setCopyDestPath("");
      if (toMove) await loadTabDirectory(tab.id, tab.currentDir);
      showNotification(toMove ? "Moved" : "Copied");
    } catch (err) { showNotification(`Error: ${err}`); }
  }

  async function handleDrop(e: DragEvent, target: "main" | "split") {
    const filePath = e.dataTransfer?.getData("text/plain");
    if (!filePath) return;
    const tab = activeTab();
    if (!tab) return;
    const targetDir = target === "main" ? tab.currentDir : tab.splitDir;
    if (!targetDir) return;
    const fileName = filePath.split("/").pop() || "";
    const destPath = targetDir === "/" ? `/${fileName}` : `${targetDir}/${fileName}`;
    try {
      await apiMove(filePath, destPath);
      setDropTarget(null);
      await loadTabDirectory(tab.id, tab.currentDir);
      if (tab.splitView) await loadTabSplitDirectory(tab.id, tab.splitDir);
      showNotification("Moved");
    } catch (err) { showNotification(`Error: ${err}`); }
  }

  function showNotification(msg: string) {
    setNotification(msg);
    setTimeout(() => setNotification(""), 2500);
  }

  function handleContextMenu(e: MouseEvent, file: any) {
    e.preventDefault();
    updateActiveTab({ selectedFile: file });
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
      items.push({ label: "Open", icon: Folder, action: () => navigateTo(file.path) });
      items.push({ label: "Open in New Tab", icon: Plus, action: () => addTab(file.path) });
      items.push({ label: "Download as ZIP", icon: Download, action: () => apiDownload(file.path) });
      items.push({ label: "", action: () => {} });
    } else {
      items.push({ label: "Open", icon: ExternalLink, action: () => {} });
      if (isImage(file)) {
        items.push({ label: "Open as Image", icon: Image, action: () => _props.onOpenImage?.(file.path) });
      }
      items.push({ label: "Download", icon: Download, action: () => apiDownload(file.path) });
      items.push({ label: "", action: () => {} });
    }
    items.push(
      { label: "Rename", icon: Pencil, action: () => { setShowRename(true); setRenameName(file.name); } },
      { label: "Copy", icon: Copy, action: () => { setShowCopyDest(true); setCopyDestPath(activeTab()?.currentDir || "/"); updateActiveTab({ selectedFile: file }); } },
      { label: "Move", icon: ExternalLink, action: () => { setShowCopyDest(true); setCopyDestPath(activeTab()?.currentDir || "/"); updateActiveTab({ selectedFile: file }); } },
      { label: "", action: () => {} },
      { label: "Delete", icon: Trash2, action: () => { updateActiveTab({ selectedFile: file }); deleteFile(); }, danger: true }
    );
    return items;
  }

  onMount(async () => {
    await loadDrives();
    const tab = activeTab();
    if (tab) {
      try {
        const home = await apiGetHome();
        await loadTabDirectory(tab.id, home.path);
      } catch {
        await loadTabDirectory(tab.id, "/");
      }
    }
  });

  const FileGridHeader = () => (
    <div class="files-grid-header">
      <div onClick={() => toggleSort("name")}></div>
      <div onClick={() => toggleSort("name")}>Name {sortBy() === "name" ? (sortAsc() ? "↑" : "↓") : ""}</div>
      <div onClick={() => toggleSort("size")}>Size {sortBy() === "size" ? (sortAsc() ? "↑" : "↓") : ""}</div>
      <div onClick={() => toggleSort("owner")}><User size={12} class="inline-icon" /> Owner {sortBy() === "owner" ? (sortAsc() ? "↑" : "↓") : ""}</div>
      <div onClick={() => toggleSort("permissions")}><ShieldCheck size={12} class="inline-icon" /> Perms {sortBy() === "permissions" ? (sortAsc() ? "↑" : "↓") : ""}</div>
      <div onClick={() => toggleSort("modified")}><Clock size={12} class="inline-icon" /> Date {sortBy() === "modified" ? (sortAsc() ? "↑" : "↓") : ""}</div>
    </div>
  );

  const FileRow = (props: { file: any; pane: "main" | "split" }) => {
    const tab = activeTab();
    return (
      <div
        class={`files-grid-row ${tab?.selectedFile?.path === props.file.path ? "selected" : ""} ${props.file.is_dir ? "folder" : "file"} ${isHidden(props.file) ? "hidden" : ""}`}
        draggable={true}
        onDragStart={(e) => { e.dataTransfer?.setData("text/plain", props.file.path); }}
        onClick={(e) => { e.stopPropagation(); updateActiveTab({ selectedFile: props.file, activePane: props.pane }); }}
        onDblClick={() => props.file.is_dir ? (props.pane === "main" ? navigateTo(props.file.path) : navigateSplitTo(props.file.path)) : isImage(props.file) ? _props.onOpenImage?.(props.file.path) : null}
        onContextMenu={(e) => handleContextMenu(e, props.file)}
      >
        <div class="file-icon-cell">
          <Show when={props.file.is_dir} fallback={isImage(props.file) ? <Image size={18} /> : <File size={18} />}>
            <Folder size={18} />
          </Show>
        </div>
        <div title={props.file.name}>{props.file.name}</div>
        <div class="file-meta-info">{props.file.is_dir ? "-" : formatSize(props.file.size)}</div>
        <div class="file-meta-info">{props.file.owner || "-"}</div>
        <div class="file-meta-info">{props.file.permissions || "-"}</div>
        <div class="file-meta-info">{formatDate(props.file.modified)}</div>
      </div>
    );
  };

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
              const Icon = item.icon;
              if (item.label === "") return <div class="context-menu-sep" />;
              return (
                <div
                  class={`context-menu-item ${item.danger ? "danger" : ""} ${item.disabled ? "disabled" : ""}`}
                  onClick={() => !item.disabled && handleContextAction(item.action)}
                >
                  <Show when={Icon}><Icon size={16} /></Show>
                  <span>{item.label}</span>
                </div>
              );
            }}
          </For>
        </div>
      </Show>

      <div class="files-tab-bar">
        <For each={tabs()}>
          {(tab) => (
            <div
              class={`files-tab ${tab.id === activeTabId() ? "active" : ""}`}
              onClick={() => switchTab(tab.id)}
            >
              <Folder size={14} />
              <span>{tabTitle(tab)}</span>
              <Show when={tabs().length > 1}>
                <span
                  class="files-tab-close"
                  onClick={(e) => { e.stopPropagation(); closeTab(tab.id); }}
                >
                  <X size={14} />
                </span>
              </Show>
            </div>
          )}
        </For>
        <div class="files-tab-add" onClick={() => addTab()} title="New Tab">
          <Plus size={16} />
        </div>
      </div>

      <div class="files-toolbar">
        <button class="btn-sm" onClick={goHomeActive} title="Home"><Home size={16} /></button>
        <button class="btn-sm" onClick={navigateUpActive} title="Up"><ArrowUp size={16} /></button>
        <button class="btn-sm" onClick={() => navigateActive(getActiveDir())} title="Refresh"><RefreshCw size={16} /></button>
        <button class={`btn-sm ${activeTab()?.splitView ? "active" : ""}`} onClick={toggleSplitView} title="Split View"><Columns2 size={16} /></button>
        <span class="path-display">{getActiveDir() || "/"}</span>
        <button class="btn-sm" onClick={() => setShowNewFolder(true)} title="New Folder"><FolderPlus size={16} /> Folder</button>
        <span class="toolbar-sep">|</span>
        <button 
          class={`btn-sm ${showHidden() ? "active" : ""}`} 
          onClick={() => setShowHidden(!showHidden())} 
          title={showHidden() ? "Hide Hidden Files" : "Show Hidden Files"}
        >
          <Show when={showHidden()} fallback={<EyeOff size={16} />}>
            <Eye size={16} />
          </Show>
          Hidden
        </button>
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
            <button class="sidebar-item" onClick={goHome}><Folder size={14} /> Home</button>
            <For each={drives()}>
              {(d) => <button class="sidebar-item" onClick={() => navigateTo(d.path)}><HardDrive size={14} /> {d.name}</button>}
            </For>
            <button class="sidebar-item" onClick={() => navigateTo("/tmp")}><File size={14} /> Temp</button>
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

          <div class="files-list" classList={{ "is-split": activeTab()?.splitView ?? false }}>
            <div 
              class="files-list-pane" 
              classList={{ "drop-target": dropTarget() === "main", "active-pane": activeTab()?.activePane === "main" }}
              onClick={() => updateActiveTab({ activePane: "main" })}
              onDragOver={(e) => { e.preventDefault(); setDropTarget("main"); }}
              onDragLeave={() => setDropTarget(null)}
              onDrop={(e) => { e.preventDefault(); handleDrop(e, "main"); }}
            >
              <div class="files-scroll-area">
                <div class="files-grid">
                  <FileGridHeader />
                  <For each={sortedFiltered(activeTab()?.files ?? [])}>
                    {(file) => <FileRow file={file} pane="main" />}
                  </For>
                </div>
              </div>
            </div>

            <Show when={activeTab()?.splitView}>
              <div 
                class="files-list-pane" 
                classList={{ "drop-target": dropTarget() === "split", "active-pane": activeTab()?.activePane === "split" }}
                onClick={() => updateActiveTab({ activePane: "split" })}
                onDragOver={(e) => { e.preventDefault(); setDropTarget("split"); }}
                onDragLeave={() => setDropTarget(null)}
                onDrop={(e) => { e.preventDefault(); handleDrop(e, "split"); }}
              >
                <div class="files-scroll-area">
                  <div class="files-grid">
                    <FileGridHeader />
                    <For each={sortedFiltered(activeTab()?.splitFiles ?? [])}>
                      {(file) => <FileRow file={file} pane="split" />}
                    </For>
                  </div>
                </div>
              </div>
            </Show>
          </div>

          <Show when={activeTab()?.selectedFile}>
            <div class="files-details">
              <div class="details-info">
                <strong>Name:</strong> <span>{activeTab()?.selectedFile?.name}</span>
                <strong>Path:</strong> <span>{activeTab()?.selectedFile?.path}</span>
                <strong>Size:</strong> <span>{activeTab()?.selectedFile?.is_dir ? "-" : formatSize(activeTab()?.selectedFile?.size)}</span>
                <strong>Modified:</strong> <span>{formatDate(activeTab()?.selectedFile?.modified)}</span>
              </div>
              <div class="details-actions">
                <button class="btn-sm" onClick={() => { setShowRename(true); setRenameName(activeTab()!.selectedFile!.name); }}>Rename</button>
                <button class="btn-sm" onClick={() => setShowCopyDest(true)}>Copy/Move</button>
                <button class="btn-sm btn-danger" onClick={deleteFile}>Delete</button>
                <button class="btn-sm" onClick={() => updateActiveTab({ selectedFile: null })}>Close</button>
              </div>
            </div>
          </Show>
        </div>
      </div>
    </div>
  );
}

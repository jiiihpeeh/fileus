import { createSignal, Show, onMount, onCleanup } from "solid-js";
import { FolderOpen, Save, FileText } from "lucide-solid";
import { apiRead, apiWrite } from "../api";
import { FileDialog, SaveDialog } from "../components/Dialogs";
import { notificationStore } from "../notificationStore";
import type { EditorInstance } from "./editorCore";
import "./TextEditor.css";

interface TextEditorProps {
  onClose: () => void;
}

export function TextEditor(_props: TextEditorProps) {
  const [path, setPath] = createSignal("");
  const [saved, setSaved] = createSignal(true);
  const [showOpenDialog, setShowOpenDialog] = createSignal(false);
  const [showSaveDialog, setShowSaveDialog] = createSignal(false);
  const [charCount, setCharCount] = createSignal(0);
  const [lineCount, setLineCount] = createSignal(1);
  const [loading, setLoading] = createSignal(true);
  let editor: EditorInstance | null = null;
  let module: typeof import("./editorCore") | null = null;
  let containerRef: HTMLDivElement | undefined;

  onMount(async () => {
    module = await import("./editorCore");
    editor = await module.createEditor(containerRef!, "", "", {
      onDirty: () => setSaved(false),
      onStats: (chars, lines) => { setCharCount(chars); setLineCount(lines); },
    });
    setLoading(false);
  });

  onCleanup(() => {
    editor?.destroy();
  });

  async function openFileCallback(filePath: string | null) {
    if (!filePath) return;
    try {
      const r = await apiRead(filePath);
      setPath(filePath);
      editor?.destroy();
      editor = await module!.createEditor(containerRef!, r.content || "", filePath, {
        onDirty: () => setSaved(false),
        onStats: (chars, lines) => { setCharCount(chars); setLineCount(lines); },
      });
      setSaved(true);
      setShowOpenDialog(false);
    } catch (err) { showNotification(`Error: ${err}`); }
  }

  async function saveFileCallback(filePath: string, fileContent: string) {
    try {
      await apiWrite(filePath, fileContent);
      setPath(filePath);
      editor?.destroy();
      editor = await module!.createEditor(containerRef!, fileContent, filePath, {
        onDirty: () => setSaved(false),
        onStats: (chars, lines) => { setCharCount(chars); setLineCount(lines); },
      });
      setSaved(true);
      setShowSaveDialog(false);
      showNotification("Saved!");
    } catch (err) { showNotification(`Error: ${err}`); }
  }

  async function handleSave() {
    const p = path();
    if (!p) { setShowSaveDialog(true); return; }
    const content = editor?.getContent() || "";
    try {
      await apiWrite(p, content);
      setSaved(true);
      showNotification("Saved!");
    } catch (err) { showNotification(`Error: ${err}`); }
  }

  function showNotification(msg: string) {
    notificationStore.showNotification(msg, 2000);
  }

  return (
    <div class="app-editor">
      <Show when={showOpenDialog()}>
        <FileDialog title="Open File" onConfirm={openFileCallback} onCancel={() => setShowOpenDialog(false)} />
      </Show>
      <Show when={showSaveDialog()}>
        <SaveDialog initialPath={path()} onConfirm={saveFileCallback} onCancel={() => setShowSaveDialog(false)} />
      </Show>
      <div class="editor-toolbar">
        <div style="display: flex; gap: 8px; align-items: center;">
          <button class="btn-sm" onClick={() => setShowOpenDialog(true)}><FolderOpen size={14} /> Open</button>
          <button class="btn-sm btn-primary" onClick={handleSave}><Save size={14} /> Save</button>
          <button class="btn-sm" onClick={() => setShowSaveDialog(true)}><FileText size={14} /> Save As</button>
        </div>
        <div style="display: flex; align-items: center; gap: 8px; flex: 1; justify-content: flex-end;">
          <span class="path-display" style="max-width: 400px;">{path() || "No file open"}</span>
          <Show when={!saved()}>
            <span class="file-meta-info" style="color: var(--warning);">● unsaved</span>
          </Show>
        </div>
      </div>
      <div class="editor-container" ref={containerRef!}>
        <Show when={loading()}>
          <div class="editor-loading">Loading editor...</div>
        </Show>
      </div>
      <div class="editor-footer">
        <span>Chars: {charCount()}</span>
        <span style="margin-left: 12px;">Lines: {lineCount()}</span>
      </div>
    </div>
  );
}

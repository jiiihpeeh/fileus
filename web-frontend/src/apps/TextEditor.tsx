import { createSignal, Show } from "solid-js";
import { FolderOpen, Save, FileText } from "lucide-solid";
import { apiRead, apiWrite } from "../api";
import { FileDialog, SaveDialog } from "../components/Dialogs";
import { notificationStore } from "../notificationStore";
import "./TextEditor.css";

interface TextEditorProps {
  onClose: () => void;
}

export function TextEditor(_props: TextEditorProps) {
  const [path, setPath] = createSignal("");
  const [content, setContent] = createSignal("");
  const [saved, setSaved] = createSignal(true);
  const [showOpenDialog, setShowOpenDialog] = createSignal(false);
  const [showSaveDialog, setShowSaveDialog] = createSignal(false);

  async function openFileCallback(filePath: string | null) {
    if (!filePath) return;
    try {
      const r = await apiRead(filePath);
      setPath(filePath);
      setContent(r.content || "");
      setSaved(true);
      setShowOpenDialog(false);
    } catch (err) { showNotification(`Error: ${err}`); }
  }

  async function saveFileCallback(filePath: string, fileContent: string) {
    try {
      await apiWrite(filePath, fileContent);
      setPath(filePath);
      setContent(fileContent);
      setSaved(true);
      setShowSaveDialog(false);
      showNotification("Saved!");
    } catch (err) { showNotification(`Error: ${err}`); }
  }

  async function handleSave() {
    const p = path();
    if (!p) { setShowSaveDialog(true); return; }
    try {
      await apiWrite(p, content());
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
      <div class="editor-container">
        <textarea
          class="editor-textarea"
          value={content()}
          onInput={(e) => { setContent((e.target as HTMLTextAreaElement).value); setSaved(false); }}
          placeholder="Open a file or start typing..."
        />
      </div>
      <div class="editor-footer">
        <span>Chars: {content().length}</span>
        <span style="margin-left: 12px;">Lines: {content().split('\n').length}</span>
      </div>
    </div>
  );
}
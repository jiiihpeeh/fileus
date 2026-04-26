import { createSignal, Show } from "solid-js";
import { apiRead, apiWrite } from "../api";
import { FileDialog, SaveDialog } from "../components/Dialogs";

interface TextEditorProps {
  onClose: () => void;
}

export function TextEditor(_props: TextEditorProps) {
  const [path, setPath] = createSignal("");
  const [content, setContent] = createSignal("");
  const [saved, setSaved] = createSignal(true);
  const [notification, setNotification] = createSignal("");
  const [showOpenDialog, setShowOpenDialog] = createSignal(false);
  const [showSaveDialog, setShowSaveDialog] = createSignal(false);

  async function openFileCallback(filePath: string) {
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
    setNotification(msg);
    setTimeout(() => setNotification(""), 2000);
  }

  return (
    <div class="app-editor">
      <Show when={notification()}>
        <div class="app-notification">{notification()}</div>
      </Show>
      <Show when={showOpenDialog()}>
        <FileDialog title="Open File" onConfirm={openFileCallback} onCancel={() => setShowOpenDialog(false)} />
      </Show>
      <Show when={showSaveDialog()}>
        <SaveDialog initialPath={path()} onConfirm={saveFileCallback} onCancel={() => setShowSaveDialog(false)} />
      </Show>
      <div class="editor-toolbar">
        <button class="btn-sm" onClick={() => setShowOpenDialog(true)}>📂 Open</button>
        <button class="btn-sm btn-primary" onClick={handleSave}>💾 Save</button>
        <button class="btn-sm" onClick={() => setShowSaveDialog(true)}>📄 Save As</button>
        <span class="editor-path">{path() || "No file open"}</span>
        <Show when={!saved()}>
          <span class="unsaved">● unsaved</span>
        </Show>
      </div>
      <textarea
        class="editor-textarea"
        value={content()}
        onInput={(e) => { setContent((e.target as HTMLTextAreaElement).value); setSaved(false); }}
        placeholder="Open a file or start typing..."
      />
    </div>
  );
}
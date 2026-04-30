import { createSignal, Show } from "solid-js";
import { FolderOpen, ImageIcon, AlertTriangle, Loader2 } from "lucide-solid";
import { FileDialog } from "../components/Dialogs";
import { apiBinary } from "../api";
import "./ImageViewer.css";

interface ImageViewerProps {
  onClose: () => void;
  pendingPath?: string;
  onImageLoaded?: () => void;
}

export function ImageViewer(props: ImageViewerProps) {
  const [imagePath, setImagePath] = createSignal("");
  const [imageUrl, setImageUrl] = createSignal("");
  const [showOpenDialog, setShowOpenDialog] = createSignal(false);
  const [error, setError] = createSignal("");
  const [loading, setLoading] = createSignal(false);

  async function loadImageDirect(path: string) {
    setLoading(true);
    setImagePath(path);
    setError("");
    try {
      const blob = await apiBinary(path);
      const url = URL.createObjectURL(blob);
      setImageUrl(url);
    } catch (err) {
      setError(`Failed to load: ${err}`);
      setImageUrl("");
    } finally {
      setLoading(false);
      props.onImageLoaded?.();
    }
  }

  if (props.pendingPath && !imagePath()) {
    loadImageDirect(props.pendingPath);
  }

  function openImage() {
    setShowOpenDialog(true);
  }

  function handleFileSelected(path: string | null) {
    setShowOpenDialog(false);
    if (path) loadImageDirect(path);
  }

  return (
    <div class="app-imageviewer" tabIndex={0}>
      <div class="proc-toolbar img-toolbar">
        <Show when={imagePath()}>
          <span class="path-display">{imagePath()}</span>
        </Show>
        <button class="btn-sm" onClick={openImage}><FolderOpen size={16} /> Open</button>
      </div>
      <div class="viewer-body">
        <Show when={loading()}>
          <div class="viewer-loading" style="display: flex; flex-direction: column; align-items: center; gap: 12px;">
            <Loader2 size={32} class="animate-spin" color="var(--accent)" />
            <p>Loading... {imagePath()}</p>
          </div>
        </Show>
        <Show when={error()}>
          <div class="viewer-empty" style="text-align: center;">
            <AlertTriangle size={48} color="var(--danger)" style="margin-bottom: 16px;" />
            <p style="margin-bottom: 16px;">{error()}</p>
            <button class="btn-sm btn-primary" onClick={openImage}>Try Again</button>
          </div>
        </Show>
        <Show when={imageUrl() && !loading()}>
          <img src={imageUrl()} alt={imagePath()} class="viewer-image" />
        </Show>
        <Show when={!imageUrl() && !loading() && !error()}>
          <div class="viewer-empty" style="text-align: center;">
            <ImageIcon size={64} color="var(--bg-tertiary)" style="margin-bottom: 16px;" />
            <p style="margin-bottom: 16px; color: var(--text-secondary);">No image open</p>
            <button class="btn-sm btn-primary" onClick={openImage}>Open Image</button>
          </div>
        </Show>
        <Show when={showOpenDialog()}>
          <FileDialog title="Open Image" onConfirm={handleFileSelected} onCancel={() => setShowOpenDialog(false)} />
        </Show>
      </div>
    </div>
  );
}
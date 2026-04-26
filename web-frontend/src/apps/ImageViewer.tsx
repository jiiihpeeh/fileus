import { createSignal, Show } from "solid-js";
import { FileDialog } from "../components/Dialogs";
import { apiBinary } from "../api";

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
      <div class="viewer-toolbar">
        <button class="btn-sm" onClick={openImage}>📂 Open</button>
        <Show when={imagePath()}>
          <span class="viewer-path">{imagePath()}</span>
        </Show>
      </div>
      <div class="viewer-body">
        <Show when={loading()}>
          <div class="viewer-loading">Loading... {imagePath()}</div>
        </Show>
        <Show when={error()}>
          <div class="viewer-empty">
            <span class="empty-icon">❌</span>
            <p>{error()}</p>
            <button class="btn-sm btn-primary" onClick={openImage}>Try Again</button>
          </div>
        </Show>
        <Show when={imageUrl() && !loading()}>
          <div class="viewer-image-container">
            <img src={imageUrl()} alt={imagePath()} class="viewer-image" />
          </div>
        </Show>
        <Show when={!imageUrl() && !loading() && !error()}>
          <div class="viewer-empty">
            <span class="empty-icon">🖼️</span>
            <p>No image open</p>
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
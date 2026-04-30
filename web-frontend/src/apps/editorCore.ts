import { EditorView, basicSetup } from "codemirror";
import { EditorState } from "@codemirror/state";
import { oneDark } from "@codemirror/theme-one-dark";

async function langFromPath(path: string) {
  const name = path.split("/").pop() || "";
  const ext = name.split(".").pop()?.toLowerCase();
  switch (ext) {
    case "js": case "mjs": case "cjs":
      return (await import("@codemirror/lang-javascript")).javascript();
    case "jsx":
      return (await import("@codemirror/lang-javascript")).javascript({ jsx: true });
    case "ts":
      return (await import("@codemirror/lang-javascript")).javascript({ typescript: true });
    case "tsx":
      return (await import("@codemirror/lang-javascript")).javascript({ typescript: true, jsx: true });
    case "html": case "htm":
      return (await import("@codemirror/lang-html")).html();
    case "css": case "scss": case "less":
      return (await import("@codemirror/lang-css")).css();
    case "json":
      return (await import("@codemirror/lang-json")).json();
    case "md": case "mdx":
      return (await import("@codemirror/lang-markdown")).markdown();
    case "py":
      return (await import("@codemirror/lang-python")).python();
    case "rs":
      return (await import("@codemirror/lang-rust")).rust();
    case "xml": case "svg": case "xhtml":
      return (await import("@codemirror/lang-xml")).xml();
    default:
      return (await import("@codemirror/lang-javascript")).javascript();
  }
}

export interface EditorCallbacks {
  onDirty?: () => void;
  onStats?: (chars: number, lines: number) => void;
}

export interface EditorInstance {
  destroy(): void;
  getContent(): string;
}

export async function createEditor(
  container: HTMLElement,
  content: string,
  path: string,
  cbs?: EditorCallbacks,
): Promise<EditorInstance> {
  const lang = await langFromPath(path);
  const state = EditorState.create({
    doc: content,
    extensions: [
      basicSetup,
      oneDark,
      lang,
      EditorView.updateListener.of((update) => {
        if (update.docChanged) {
          cbs?.onDirty?.();
          cbs?.onStats?.(update.state.doc.length, update.state.doc.lines);
        }
      }),
    ],
  });
  const view = new EditorView({ state, parent: container });
  cbs?.onStats?.(content.length, content ? content.split("\n").length : 1);
  return {
    destroy: () => view.destroy(),
    getContent: () => view.state.doc.toString(),
  };
}

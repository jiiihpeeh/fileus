import { createSignal, For } from "solid-js";
import "./styles.css";

export default function KeyInput(props: { onComplete: (key: string) => void }) {
  const [chars, setChars] = createSignal<string[]>(Array(10).fill(""));

  function handlePaste(e: ClipboardEvent) {
    e.preventDefault();
    const pasted = e.clipboardData?.getData("text") || "";
    const filtered = pasted.replace(/[^A-Za-z0-9]/g, "").slice(0, 10);
    const newChars = filtered.split("").concat(Array(10 - filtered.length).fill(""));
    setChars(newChars);
    if (filtered.length === 10) {
      props.onComplete(filtered);
    }
  }

  function handleInput(index: number, value: string) {
    const filtered = value.replace(/[^A-Za-z0-9]/g, "").slice(-1);
    const newChars = [...chars()];
    newChars[index] = filtered;
    setChars(newChars);
    if (filtered && index < 9) {
      const next = document.querySelectorAll('.key-char-input')[index + 1] as HTMLInputElement;
      next?.focus();
    }
    const key = newChars.join("");
    const isComplete = key.length === 10 && key.split("").every(c => c !== "");
    if (isComplete) {
      props.onComplete(key);
    }
  }

  return (
    <div class="key-slots">
      <For each={chars()}>{(c, i) =>
        <input
          class="key-char-input"
          type="text"
          maxLength={1}
          value={c}
          onInput={(e) => handleInput(i(), (e.target as HTMLInputElement).value)}
          onPaste={handlePaste}
        />
      }</For>
    </div>
  );
}
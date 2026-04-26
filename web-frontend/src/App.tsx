import { createSignal, Switch, Match } from "solid-js";
import { Desktop } from "./apps/Desktop";
import { generateNewKey, encryptSession } from "./crypto";
import { setSessionKey } from "./api";
import "./styles.css";

function KeyInput(props: { onComplete: (key: string) => void }) {
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
      {chars().map((c, i) => (
        <input
          class="key-char-input"
          type="text"
          maxLength={1}
          value={c}
          onInput={(e) => handleInput(i, (e.target as HTMLInputElement).value)}
          onPaste={handlePaste}
        />
      ))}
    </div>
  );
}

type AppStatus = "entering" | "verifying" | "verified" | "error";

export default function App() {
  const [status, setStatus] = createSignal<AppStatus>("entering");
  const [errorMessage, setErrorMessage] = createSignal("");

  async function handleKeyComplete(key: string) {
    setStatus("verifying");
    setErrorMessage("");
    try {
      const newKey = generateNewKey();
      const encrypted = await encryptSession(newKey, key);
      
      const response = await fetch('/api/session/verify', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ data: encrypted }),
      });
      
      const result = await response.json();
      
      if (result.valid) {
        setSessionKey(newKey);
        setStatus("verified");
      } else {
        setErrorMessage("Invalid shared key");
        setStatus("error");
      }
    } catch (e) {
      setErrorMessage("Verification failed");
      setStatus("error");
    }
  }

  return (
    <Switch fallback={<div>Loading...</div>}>
      <Match when={status() === "verified"}>
        <div class="desktop"><Desktop /></div>
      </Match>
      <Match when={status() === "verifying"}>
        <div class="key-entry">
          <h1>Verifying...</h1>
          <p>Please wait</p>
        </div>
      </Match>
      <Match when={status() === "error"}>
        <div class="key-entry">
          <h1>Enter Shared Key</h1>
          <p>Paste or type your 10-character key</p>
          <KeyInput onComplete={handleKeyComplete} />
          <p class="error">{errorMessage()}</p>
        </div>
      </Match>
      <Match when={status() === "entering"}>
        <div class="key-entry">
          <h1>Enter Shared Key</h1>
          <p>Paste or type your 10-character key</p>
          <KeyInput onComplete={handleKeyComplete} />
        </div>
      </Match>
    </Switch>
  );
}
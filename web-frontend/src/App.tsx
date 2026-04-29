import { createSignal, Switch, Match } from "solid-js";
import { Desktop } from "./apps/Desktop";
import { generateNewKey, encryptSession } from "./crypto";
import { setSessionKey } from "./api";
import "./styles.css";
import KeyInput from "./KeyInput";


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
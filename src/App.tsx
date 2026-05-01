import { createSignal, onMount } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import Window from "./components/Window";
import "./styles/themes.css";
import "./styles/base.css";
import "./styles/components.css";

function loadTheme(): string {
  try {
    const v = localStorage.getItem("fileus-theme");
    return v ? JSON.parse(v) : "dark-red";
  } catch {
    return "dark-red";
  }
}

const initialTheme = loadTheme();
document.documentElement.setAttribute("data-theme", initialTheme);

function App() {
  const [greetMsg, setGreetMsg] = createSignal("");
  const [name, setName] = createSignal("");
  const [status, setStatus] = createSignal("Idle");
  const [certsInfo, setCertsInfo] = createSignal("Not generated");
  const [serverStatus, setServerStatus] = createSignal("Starting...");
  const [serverPort, setServerPort] = createSignal(8080);
  const [customPort, setCustomPort] = createSignal("");
  const [sharedKey, setSharedKey] = createSignal<string | null>(null);
  const [connected, setConnected] = createSignal(false);

  async function checkServer() {
    try {
      const running = await invoke("is_server_running") as boolean;
      setServerStatus(running ? "Running" : "Stopped");
      return running;
    } catch {
      setServerStatus("Unknown");
      return false;
    }
  }

  async function refreshPort() {
    try {
      const port = await invoke("get_server_port") as number;
      setServerPort(port);
    } catch {}
  }

  async function fetchSharedKey() {
    try {
      const key = await invoke("get_shared_key") as string | null;
      console.log("fetchSharedKey result:", key);
      setSharedKey(key);
      if (key) {
        const sessionKey = await invoke("get_session_new_key") as string | null;
        if (sessionKey) {
          setConnected(true);
        }
      }
    } catch (e) {
      console.error("fetchSharedKey error:", e);
    }
  }

  onMount(async () => {
    setStatus("Generating TLS certificates...");
    try {
      const result = await invoke("generate_tls_certificates");
      const typedResult = result as { domain: string; ca_cert: string; domain_cert: string };
      setStatus(`Certs: ${typedResult.domain} OK`);
      setCertsInfo(`${typedResult.domain} | CA + Domain cert generated`);
      await invoke("set_random_shared_alphanumeric_key");
      await refreshPort();
      await fetchSharedKey();
      setTimeout(async () => {
        try {
          await fetch(`http://localhost:${serverPort()}/api/health`);
          const running = await checkServer();
          setServerStatus(running ? "Running" : "Unknown");
        } catch {
          await checkServer();
        }
      }, 1000);
    } catch (err) {
      setStatus(`Error: ${String(err)}`);
      setCertsInfo("Failed to generate");
    }
  });

  async function greet() {
    try {
      const msg = await invoke("greet", { name: name() });
      setGreetMsg(msg as string);
    } catch (err) {
      setGreetMsg(String(err));
    }
  }

  async function toggleServer() {
    const port = customPort() ? parseInt(customPort()) : serverPort();
    if (isNaN(port) || port < 1 || port > 65535) {
      alert("Invalid port number (1-65535)");
      return;
    }
    try {
      const result = await invoke("toggle_http_server", { port }) as string;
      setServerStatus(result);
    } catch (err) {
      alert(String(err));
    }
  }

  async function applyPort() {
    const port = parseInt(customPort() || "0");
    if (isNaN(port) || port < 1 || port > 65535) {
      alert("Invalid port number (1-65535)");
      return;
    }
    const running = await checkServer();
    if (running) {
      alert("Stop server first before changing port");
      return;
    }
    await invoke("set_server_port", { port });
    await refreshPort();
    setCustomPort("");
  }

  return (
    <Window title="Fileus">
      <main class="container">
        <h1>
          <span class="badge tauri">Tauri</span>
          Fileus
        </h1>
        <p>Pure Rust HTTP Server | Zero JS Runtime</p>

        <div class="card">
          <h2>Server</h2>
          <p><strong>HTTP:</strong> <a href={`http://localhost:${serverPort()}`} target="_blank" rel="noopener noreferrer">{`http://localhost:${serverPort()}`}</a></p>
          <p><strong>API Health:</strong> <a href={`http://localhost:${serverPort()}/api/health`} target="_blank" rel="noopener noreferrer">/api/health</a></p>
          <p><strong>API Greet:</strong> <a href={`http://localhost:${serverPort()}/api/greet?name=Tauri`} target="_blank" rel="noopener noreferrer">/api/greet?name=Tauri</a></p>
          <p>
            <strong>Status:</strong>{" "}
            <span class="status-text" classList={{ on: serverStatus() === "Running", off: serverStatus() !== "Running" }}>
              {serverStatus()}
            </span>
          </p>
          {connected() ? (
            <p class="connected-text">Connected</p>
          ) : (
            <div class="key-display-row">
              <div class="key-display">
                {(sharedKey() || "----------").split("").map((c) => (
                  <span class="key-char">{c}</span>
                ))}
              </div>
              <button class="copy-btn" onClick={() => sharedKey() && navigator.clipboard.writeText(sharedKey()!)}>Copy</button>
            </div>
          )}
          <div class="row" style={{ "margin-top": "12px" }}>
            <input
              type="number"
              placeholder={`Port (current: ${serverPort()})`}
              value={customPort()}
              onInput={(e) => setCustomPort((e.currentTarget as HTMLInputElement).value)}
            />
            <button onClick={applyPort}>Set Port</button>
            <div class="toggle-row">
              <span>Server</span>
              <label class="toggle-switch">
                <input
                  type="checkbox"
                  checked={serverStatus() === "Running"}
                  onChange={toggleServer}
                />
                <span class="toggle-slider"></span>
              </label>
              <span class="status-text" classList={{ on: serverStatus() === "Running", off: serverStatus() !== "Running" }}>
                {serverStatus() === "Running" ? "ON" : "OFF"}
              </span>
            </div>
          </div>
        </div>

        <div class="card">
          <h2>TLS Certificates</h2>
          <p>{certsInfo()}</p>
          <p><em>Generated by Rust (rcgen) on app startup</em></p>
        </div>

        <div class="card">
          <h2>Tauri Bridge</h2>
          <p>{status()}</p>
          <p>Test the Rust command invocation:</p>
          <div class="row">
            <input
              id="nameInput"
              type="text"
              placeholder="Enter a name..."
              value={name()}
              onInput={(e) => setName((e.currentTarget as HTMLInputElement).value)}
            />
            <button onClick={greet}>Greet</button>
          </div>
          {greetMsg() && <p class="result">{greetMsg()}</p>}
        </div>
      </main>
    </Window>
  );
}

export default App;

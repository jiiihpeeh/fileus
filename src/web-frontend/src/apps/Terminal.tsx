import { createSignal, onMount, onCleanup } from "solid-js";
import { Terminal as XTerm } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";

interface TerminalProps {
  onClose: () => void;
}

export function Terminal(_props: TerminalProps) {
  let terminalRef: HTMLDivElement | undefined;
  let term: XTerm | undefined;
  let fitAddon: FitAddon | undefined;

  const [cwd, setCwd] = createSignal("/");
  let inputBuffer = "";

  onMount(() => {
    if (!terminalRef) return;

    term = new XTerm({
      cursorBlink: true,
      fontSize: 14,
      fontFamily: "'Consolas', 'Monaco', monospace",
      theme: {
        background: "#0a0a0f",
        foreground: "#eaeaea",
        cursor: "#e94560",
        black: "#1a1a2e",
        red: "#e94560",
        green: "#4ecca3",
        yellow: "#f9cd7d",
        blue: "#8be9fd",
        magenta: "#ff79c6",
        cyan: "#50fa7b",
        white: "#eaeaea",
      },
    });

    fitAddon = new FitAddon();
    term.loadAddon(fitAddon);
    term.open(terminalRef);
    fitAddon.fit();

    term.writeln("\x1b[36mWelcome to Fileus Terminal\x1b[0m");
    term.writeln("Type 'help' for available commands\r\n");

    function printPrompt() {
      if (term) {
        term.write(`\r\n\x1b[32m${cwd()}\x1b[0m$ `);
      }
    }

    printPrompt();

    term.onData((data) => {
      const code = data.charCodeAt(0);

      if (code === 13) {
        term?.write("\r\n");
        const cmd = inputBuffer.trim();
        inputBuffer = "";

        if (cmd) {
          processCommand(cmd);
        }
        printPrompt();
        return;
      }

      if (code === 127) {
        if (inputBuffer.length > 0) {
          inputBuffer = inputBuffer.slice(0, -1);
          term?.write("\b \b");
        }
        return;
      }

      if (code < 32) {
        return;
      }

      inputBuffer += data;
      term?.write(data);
    });

    function processCommand(cmd: string) {
      if (cmd === "clear") {
        term?.clear();
        return;
      }

      if (cmd === "help") {
        term?.writeln("\x1b[33mAvailable commands:\x1b[0m");
        term?.writeln("  help     - Show this help");
        term?.writeln("  clear    - Clear terminal");
        term?.writeln("  pwd      - Print working directory");
        term?.writeln("  ls       - List files");
        term?.writeln("  cd <dir> - Change directory");
        term?.writeln("  cat <f>  - Read file");
        term?.writeln("");
        return;
      }

      if (cmd === "pwd") {
        term?.writeln(`\x1b[32m${cwd()}\x1b[0m`);
        return;
      }

      if (cmd.startsWith("cd ")) {
        const dir = cmd.substring(3).trim();
        if (dir === "..") {
          const parts = cwd().split("/").filter(Boolean);
          setCwd(parts.length <= 1 ? "/" : "/" + parts.slice(0, -1).join("/"));
        } else if (dir === "/") {
          setCwd("/");
        } else {
          setCwd(cwd() === "/" ? "/" + dir : cwd() + "/" + dir);
        }
        return;
      }

      if (cmd === "ls") {
        term?.writeln("\x1b[35mListing files in " + cwd() + "...\x1b[0m");
        term?.writeln("(Use File Browser for full file listing)\r\n");
        return;
      }

      if (cmd.startsWith("cat ")) {
        term?.writeln("\x1b[33mFile reading not implemented in demo\x1b[0m");
        return;
      }

      term?.writeln(`\x1b[31mCommand not found: ${cmd}\x1b[0m`);
      term?.writeln("Type 'help' for available commands");
    }

    const resizeObserver = new ResizeObserver(() => {
      fitAddon?.fit();
    });
    resizeObserver.observe(terminalRef);

    onCleanup(() => {
      resizeObserver.disconnect();
      term?.dispose();
    });
  });

  return (
    <div class="app-terminal">
      <div ref={terminalRef} class="terminal-container"></div>
    </div>
  );
}
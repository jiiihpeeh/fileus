import { createSignal, For, onMount } from "solid-js";
import { RefreshCw, Search, Cpu, Activity, LayoutList } from "lucide-solid";
import { apiGetProcesses, formatSize } from "../api";
import "./ProcessManager.css";

interface ProcessManagerProps {
  onClose: () => void;
}

export function ProcessManager(_props: ProcessManagerProps) {
  const [processes, setProcesses] = createSignal<any[]>([]);
  const [sortBy, setSortBy] = createSignal<"pid" | "name" | "cpu" | "memory">("cpu");
  const [sortAsc, setSortAsc] = createSignal(false);
  const [search, setSearch] = createSignal("");
  const [loading, setLoading] = createSignal(false);

  async function loadProcesses() {
    setLoading(true);
    try {
      const data = await apiGetProcesses();
      setProcesses(data || []);
    } catch (err) {
      console.error("Failed to load processes:", err);
    } finally {
      setLoading(false);
    }
  }

  function filtered() {
    let p = [...processes()];
    const q = search().toLowerCase();
    if (q) p = p.filter(p => (p.name || "").toLowerCase().includes(q));
    const by = sortBy();
    const asc = sortAsc();
    p.sort((a, b) => {
      let cmp = 0;
      if (by === "name") cmp = (a.name || "").localeCompare(b.name || "");
      else if (by === "pid") cmp = (a.pid || 0) - (b.pid || 0);
      else if (by === "cpu") cmp = (a.cpu || 0) - (b.cpu || 0);
      else if (by === "memory") cmp = (a.memory || 0) - (b.memory || 0);
      return asc ? cmp : -cmp;
    });
    return p;
  }

  function toggleSort(col: string) {
    if (sortBy() === col) setSortAsc(!sortAsc());
    else { setSortBy(col as any); setSortAsc(false); }
  }

  function SortIcon(props: { col: string }) {
    if (sortBy() !== props.col) return null;
    return <span>{sortAsc() ? " ↑" : " ↓"}</span>;
  }

  onMount(() => {
    loadProcesses();
    const interval = setInterval(loadProcesses, 5000);
    return () => clearInterval(interval);
  });

  return (
    <div class="app-processes">
      <div class="proc-toolbar">
        <div style="display: flex; align-items: center; gap: 8px; flex: 1;">
          <Search size={14} color="var(--text-secondary)" />
          <input class="input" style="flex: 1;" placeholder="Search processes..." value={search()}
            onInput={(e) => setSearch((e.target as HTMLInputElement).value)} />
        </div>
        <div style="display: flex; align-items: center; gap: 12px;">
          <span class="file-meta-info"><LayoutList size={12} class="inline-icon" /> {processes().length}</span>
          <button class="btn-sm" onClick={loadProcesses} disabled={loading()} title="Refresh">
            <RefreshCw size={14} class={loading() ? "animate-spin" : ""} />
          </button>
        </div>
      </div>
      <div class="proc-table-container">
        <table class="proc-table">
          <thead>
            <tr>
              <th onClick={() => toggleSort("pid")}>PID <SortIcon col="pid" /></th>
              <th onClick={() => toggleSort("name")}>Process Name <SortIcon col="name" /></th>
              <th onClick={() => toggleSort("cpu")}><Activity size={12} class="inline-icon" /> CPU % <SortIcon col="cpu" /></th>
              <th onClick={() => toggleSort("memory")}><Cpu size={12} class="inline-icon" /> Memory <SortIcon col="memory" /></th>
            </tr>
          </thead>
          <tbody>
            <For each={filtered()}>
              {(p) => (
                <tr>
                  <td class="file-meta-info" style="width: 80px;">{p.pid}</td>
                  <td style="font-weight: 500;">{p.name}</td>
                  <td style="width: 120px;">
                    <div style="display: flex; align-items: center; gap: 8px;">
                      <span style="min-width: 35px;">{p.cpu.toFixed(1)}%</span>
                      <div class="proc-cpu-bar" style="flex: 1;">
                        <div class="proc-cpu-fill" style={{ width: `${Math.min(p.cpu, 100)}%` }}></div>
                      </div>
                    </div>
                  </td>
                  <td class="file-meta-info" style="width: 120px;">{formatSize(p.memory)}</td>
                </tr>
              )}
            </For>
          </tbody>
        </table>
      </div>
    </div>
  );
}
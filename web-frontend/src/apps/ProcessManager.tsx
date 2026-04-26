import { createSignal, For, onMount } from "solid-js";
import { apiGetProcesses, formatSize } from "../api";

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

  function sortIcon(col: string) {
    if (sortBy() !== col) return "";
    return sortAsc() ? " ↑" : " ↓";
  }

  onMount(loadProcesses);

  return (
    <div class="app-processes">
      <div class="proc-toolbar">
        <input class="input" placeholder="Search by name..." value={search()}
          onInput={(e) => setSearch((e.target as HTMLInputElement).value)} />
        <button class="btn-sm" onClick={loadProcesses} disabled={loading()}>
          {loading() ? "..." : "⟳"}
        </button>
        <span class="proc-count">{processes().length} processes</span>
      </div>
      <table class="proc-table">
        <thead>
          <tr>
            <th onClick={() => toggleSort("pid")}>PID{sortIcon("pid")}</th>
            <th onClick={() => toggleSort("name")}>Name{sortIcon("name")}</th>
            <th onClick={() => toggleSort("cpu")}>CPU%{sortIcon("cpu")}</th>
            <th onClick={() => toggleSort("memory")}>Memory{sortIcon("memory")}</th>
          </tr>
        </thead>
        <tbody>
          <For each={filtered()}>
            {(p) => (
              <tr>
                <td class="proc-pid">{p.pid}</td>
                <td class="proc-name" title={p.name}>{p.name}</td>
                <td class="proc-cpu">{p.cpu.toFixed(1)}</td>
                <td class="proc-mem">{formatSize(p.memory)}</td>
              </tr>
            )}
          </For>
        </tbody>
      </table>
    </div>
  );
}
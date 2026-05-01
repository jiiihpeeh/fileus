import { encode, decode } from "@msgpack/msgpack";
import { encryptApiMessage, decryptApiMessage, decryptApiBinaryResponse, decryptApiBinarySimple } from "./crypto";
import { storeChunk, deleteChunks, reconstructFile, reconstructFileStream } from "./chunkStorage";

let sessionKey: string | null = null;

export function setSessionKey(key: string) {
  sessionKey = key;
}

export function clearSessionKey() {
  sessionKey = null;
}

function getSessionKey(): string {
  if (!sessionKey) throw new Error("No session key");
  return sessionKey;
}

export interface FileItem {
  name: string;
  path: string;
  is_dir: boolean;
  size: number;
  modified: number | null;
}

export interface ListResponse {
  items: FileItem[];
}

export interface InfoResponse {
  name: string;
  path: string;
  is_dir: boolean;
  size: number;
  modified: number | null;
}

export interface ReadResponse {
  content: Uint8Array;
  mime: string;
  binary: true;
  compression?: string;
}

export interface SuccessResponse {
  success: true;
}

export interface ProcessInfo {
  pid: number;
  name: string;
  cpu: number;
  memory: number;
}

export interface HomeResponse {
  path: string;
}

export interface DriveItem {
  name: string;
  path: string;
  is_dir: boolean;
  size: number;
  modified: number | null;
}

export interface ChunkMetadata {
  filename: string;
  chunk_index: number;
  total_chunks: number;
  file_size: number;
  chunk_size: number;
}

export interface SessionVerifyResponse {
  valid: boolean;
}

export interface SessionDecryptResponse {
  payload: string;
}

export interface HealthResponse {
  status: string;
  timestamp: number;
}

export interface ErrorResponse {
  error: string;
  status: number;
}

export interface GreetResponse {
  message: string;
}

export interface ChunkPayload {
  path: string;
  chunk_index?: number;
  total_chunks?: number;
}

export interface ListPayload {
  dir: string;
}

export interface SearchPayload {
  dir: string;
  pattern: string;
}

export interface WritePayload {
  path: string;
  content: string;
}

export interface CreateDirPayload {
  path: string;
}

export interface DeletePayload {
  path: string;
}

export interface RenamePayload {
  old_path: string;
  new_path: string;
}

export interface CopyPayload {
  source: string;
  dest: string;
}

export interface MovePayload {
  source: string;
  dest: string;
}

export interface DownloadPayload {
  path: string;
}

export interface HomePayload {}

export interface DrivesPayload {}

export interface ProcessesPayload {}

async function encryptedRequest<T>(path: string, payload: object): Promise<T> {
  const startTime = performance.now();
  const payloadBytes = encode(payload);
  const encrypted = await encryptApiMessage(payloadBytes, getSessionKey());
  
  console.debug(`[API] → ${path}`, { payload, encryptedLength: encrypted.length });
  
  // Send MessagePack body with binary data: {data: encryptedBytes}
  const body = encode({ data: Array.from(encrypted) });
  
  const r = await fetch(path, {
    method: "POST",
    headers: { "Content-Type": "application/msgpack" },
    body,
  });
  
  const resultBytes = new Uint8Array(await r.arrayBuffer());
  const elapsed = Math.round(performance.now() - startTime);
  
  if (!r.ok) {
    console.error(`[API] ✗ ${path} (${elapsed}ms)`, { status: r.status });
    throw new Error(`HTTP ${r.status}`);
  }
  
  try {
    // Parse MessagePack response
    const parsed = decode(resultBytes) as any;
    
    // Check if it's an error response
    if (parsed && parsed.error) {
      console.error(`[API] ✗ ${path} (${elapsed}ms)`, parsed);
      throw new Error(parsed.error);
    }
    
    // Check if there's encrypted data to decrypt
    if (parsed && parsed.data) {
      const encryptedData = parsed.data instanceof Uint8Array 
        ? parsed.data 
        : new Uint8Array(parsed.data);
      const decrypted = await decryptApiMessage(encryptedData, getSessionKey());
      const result = decode(decrypted) as T;
      console.debug(`[API] ← ${path} (${elapsed}ms)`, result);
      return result;
    }
    
    console.debug(`[API] ← ${path} (${elapsed}ms)`, parsed);
    return parsed as T;
  } catch (e) {
    console.error(`[API] ✗ ${path} (${elapsed}ms)`, { error: e });
    throw e;
  }
}

export async function apiList(dir: string): Promise<ListResponse> {
  return encryptedRequest("/api/files/list", { dir });
}

async function decompressGzip(data: Uint8Array): Promise<Uint8Array> {
  const stream = new Response(data as unknown as BodyInit).body!.pipeThrough(new DecompressionStream('gzip'));
  return new Uint8Array(await new Response(stream).arrayBuffer());
}

export async function apiRead(path: string): Promise<{ content: string; mime: string; binary: true }> {
  const r = await encryptedRequest<ReadResponse>("/api/files/read", { path });
  let bytes = r.content || new Uint8Array();
  if (r.compression === "gzip") {
    bytes = await decompressGzip(bytes);
  }
  const content = new TextDecoder().decode(bytes);
  return { mime: r.mime, binary: r.binary, content };
}

export async function apiInfo(path: string): Promise<InfoResponse> {
  return encryptedRequest("/api/files/info", { path });
}

export async function apiSearch(dir: string, pattern: string): Promise<FileItem[]> {
  return encryptedRequest("/api/files/search", { dir, pattern });
}

export async function apiWrite(path: string, content: string): Promise<SuccessResponse> {
  return encryptedRequest("/api/files/write", { path, content });
}

export async function apiCreateDir(path: string): Promise<SuccessResponse> {
  return encryptedRequest("/api/files/create-dir", { path });
}

export async function apiDelete(path: string): Promise<SuccessResponse> {
  return encryptedRequest("/api/files/delete", { path });
}

export async function apiRename(oldPath: string, newPath: string): Promise<SuccessResponse> {
  return encryptedRequest("/api/files/rename", { old_path: oldPath, new_path: newPath });
}

export async function apiCopy(source: string, dest: string): Promise<SuccessResponse> {
  return encryptedRequest("/api/files/copy", { source, dest });
}

export async function apiMove(source: string, dest: string): Promise<SuccessResponse> {
  return encryptedRequest("/api/files/move", { source, dest });
}

export async function apiGetHome(): Promise<HomeResponse> {
  return encryptedRequest("/api/system/home", {});
}

export async function apiGetDrives(): Promise<DriveItem[]> {
  return encryptedRequest("/api/system/drives", {});
}

export async function apiGetProcesses(): Promise<ProcessInfo[]> {
  return encryptedRequest("/api/system/processes", {});
}

export async function apiBinary(path: string): Promise<Blob> {
  const startTime = performance.now();
  const payloadBytes = encode({ path });
  const encrypted = await encryptApiMessage(payloadBytes, getSessionKey());
  
  console.debug(`[API] → /api/files/binary`, { path, encryptedLength: encrypted.length });
  
  // Send MessagePack body with binary data
  const body = encode({ data: Array.from(encrypted) });
  
  const r = await fetch("/api/files/binary", {
    method: "POST",
    headers: { "Content-Type": "application/msgpack" },
    body,
  });
  
  if (!r.ok) {
    console.error(`[API] ✗ /api/files/binary (${Math.round(performance.now() - startTime)}ms)`, { status: r.status });
    throw new Error(`HTTP ${r.status}`);
  }
  
  const arrayBuffer = await r.arrayBuffer();
  const decrypted = await decryptApiBinarySimple(arrayBuffer, getSessionKey());
  
  const elapsed = Math.round(performance.now() - startTime);
  console.debug(`[API] ← /api/files/binary (${elapsed}ms)`, { size: decrypted.length });
  return new Blob([new Uint8Array(decrypted)]);
}

export async function apiDownload(path: string): Promise<void> {
  const startTime = performance.now();
  const payloadBytes = encode({ path });
  const encrypted = await encryptApiMessage(payloadBytes, getSessionKey());
  
  console.debug(`[API] → /api/files/download`, { path, encryptedLength: encrypted.length });
  
  // Send MessagePack body with binary data
  const body = encode({ data: Array.from(encrypted) });
  
  const response = await fetch("/api/files/download", {
    method: "POST",
    headers: { "Content-Type": "application/msgpack" },
    body,
  });
  
  if (!response.ok) {
    console.error(`[API] ✗ /api/files/download (${Math.round(performance.now() - startTime)}ms)`, { status: response.status });
    throw new Error(`HTTP ${response.status}`);
  }
  
  const blob = await response.blob();
  const elapsed = Math.round(performance.now() - startTime);
  console.debug(`[API] ← /api/files/download (${elapsed}ms)`, { size: blob.size, type: blob.type });
  
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = path.split("/").pop() || "download";
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
}

const LARGE_FILE_THRESHOLD = 100 * 1024 * 1024;

export async function apiDownloadChunked(
  path: string,
  onProgress?: (loaded: number, total: number) => void
): Promise<void> {
  const startTime = performance.now();
  await deleteChunks(path);
  
  const payloadBytes = encode({ path, chunk_index: 0, total_chunks: 1 });
  const encrypted = await encryptApiMessage(payloadBytes, getSessionKey());
  
  console.debug(`[API] → /api/fileupload/binary`, { path, chunk_index: 0, encryptedLength: encrypted.length });
  
  // Send MessagePack body with binary data
  const body = encode({ data: Array.from(encrypted) });
  
  const r = await fetch("/api/fileupload/binary", {
    method: "POST",
    headers: { "Content-Type": "application/msgpack" },
    body,
  });
  
  if (!r.ok) {
    console.error(`[API] ✗ /api/fileupload/binary (${Math.round(performance.now() - startTime)}ms)`, { status: r.status });
    throw new Error(`HTTP ${r.status}`);
  }
  
  const arrayBuffer = await r.arrayBuffer();
  const { metadata } = await decryptApiBinaryResponse(arrayBuffer, getSessionKey());
  const totalChunks = metadata.total_chunks;
  const fileSize = metadata.file_size;
  const filename = metadata.filename || path.split("/").pop() || "download";
  
  console.debug(`[API] ← /api/fileupload/binary (${Math.round(performance.now() - startTime)}ms)`, { metadata });
  
  for (let i = 0; i < totalChunks; i++) {
    const p = encode({ path, chunk_index: i, total_chunks: totalChunks });
    const enc = await encryptApiMessage(p, getSessionKey());
    
    const chunkBody = encode({ data: Array.from(enc) });
    const res = await fetch("/api/fileupload/binary", {
      method: "POST",
      headers: { "Content-Type": "application/msgpack" },
      body: chunkBody,
    });
    
    if (!res.ok) {
      console.error(`[API] ✗ chunk ${i}/${totalChunks} (${Math.round(performance.now() - startTime)}ms)`, { status: res.status });
      throw new Error(`HTTP ${res.status}`);
    }
    
    const buf = await res.arrayBuffer();
    const { metadata: meta, payload } = await decryptApiBinaryResponse(buf, getSessionKey());
    
    await storeChunk({
      id: `${path}:${i}`,
      filePath: path,
      chunkIndex: meta.chunk_index,
      totalChunks: meta.total_chunks,
      fileSize: meta.file_size,
      data: payload.slice(0).buffer,
    });
    
    console.debug(`[API] chunk ${i + 1}/${totalChunks} stored`, { chunk_size: meta.chunk_size });
    onProgress?.(i + 1, totalChunks);
  }
  
  if (fileSize > LARGE_FILE_THRESHOLD && "showSaveFilePicker" in window) {
    console.debug(`[API] streaming large file (${fileSize} bytes) to disk`);
    const handle = await (window as any).showSaveFilePicker({
      suggestedName: filename,
    });
    const writable = handle.createWritable();
    await reconstructFileStream(path, writable, totalChunks, onProgress);
  } else {
    console.debug(`[API] reconstructing file from ${totalChunks} chunks`);
    const blob = await reconstructFile(path);
    if (!blob) throw new Error("Failed to reconstruct file");
    
    console.debug(`[API] ← /api/fileupload/binary complete (${Math.round(performance.now() - startTime)}ms)`, { size: blob.size });
    
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = filename;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
  }
  
  await deleteChunks(path);
  console.debug(`[API] ✓ /api/fileupload/binary finished (${Math.round(performance.now() - startTime)}ms total)`);
}

export function formatSize(bytes: number): string {
  if (bytes === 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.floor(Math.log(bytes) / Math.log(1024));
  return `${(bytes / Math.pow(1024, i)).toFixed(1)} ${units[i]}`;
}

export function formatDate(ts?: number): string {
  if (!ts) return "-";
  return new Date(ts * 1000).toLocaleString();
}

export async function apiSessionVerify(encryptedData: Uint8Array): Promise<SessionVerifyResponse> {
  const body = encode({ data: Array.from(encryptedData) });
  
  const r = await fetch(`/api/session/verify`, {
    method: "POST",
    headers: { "Content-Type": "application/msgpack" },
    body,
  });
  if (!r.ok) throw new Error(await r.text());
  
  const resultBytes = new Uint8Array(await r.arrayBuffer());
  return decode(resultBytes) as SessionVerifyResponse;
}

export async function apiSessionDecrypt(encryptedData: Uint8Array): Promise<SessionDecryptResponse> {
  const body = encode({ data: Array.from(encryptedData) });
  
  const r = await fetch(`/api/session/decrypt`, {
    method: "POST",
    headers: { "Content-Type": "application/msgpack" },
    body,
  });
  if (!r.ok) throw new Error(await r.text());
  
  const resultBytes = new Uint8Array(await r.arrayBuffer());
  return decode(resultBytes) as SessionDecryptResponse;
}

const DB_NAME = "FileusChunks";
const DB_VERSION = 1;
const STORE_NAME = "chunks";

function openDB(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(DB_NAME, DB_VERSION);
    request.onerror = () => reject(request.error);
    request.onsuccess = () => resolve(request.result);
    request.onupgradeneeded = (event) => {
      const db = (event.target as IDBOpenDBRequest).result;
      if (!db.objectStoreNames.contains(STORE_NAME)) {
        db.createObjectStore(STORE_NAME, { keyPath: "id" });
      }
    };
  });
}

export interface ChunkInfo {
  id: string;
  filePath: string;
  chunkIndex: number;
  totalChunks: number;
  fileSize: number;
  data: ArrayBuffer;
}

export async function storeChunk(chunk: ChunkInfo): Promise<void> {
  const db = await openDB();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(STORE_NAME, "readwrite");
    const store = tx.objectStore(STORE_NAME);
    const request = store.put(chunk);
    request.onerror = () => reject(request.error);
    request.onsuccess = () => resolve();
    db.close();
  });
}

export async function getChunk(filePath: string, chunkIndex: number): Promise<ChunkInfo | undefined> {
  const db = await openDB();
  const id = `${filePath}:${chunkIndex}`;
  return new Promise((resolve, reject) => {
    const tx = db.transaction(STORE_NAME, "readonly");
    const store = tx.objectStore(STORE_NAME);
    const request = store.get(id);
    request.onerror = () => reject(request.error);
    request.onsuccess = () => resolve(request.result);
    db.close();
  });
}

export async function getAllChunks(filePath: string): Promise<ChunkInfo[]> {
  const db = await openDB();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(STORE_NAME, "readonly");
    const store = tx.objectStore(STORE_NAME);
    const request = store.getAll();
    request.onerror = () => reject(request.error);
    request.onsuccess = () => {
      const chunks = request.result.filter((c: ChunkInfo) => c.filePath === filePath);
      resolve(chunks.sort((a: ChunkInfo, b: ChunkInfo) => a.chunkIndex - b.chunkIndex));
    };
    db.close();
  });
}

export async function deleteChunks(filePath: string): Promise<void> {
  const db = await openDB();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(STORE_NAME, "readwrite");
    const store = tx.objectStore(STORE_NAME);
    const request = store.getAll();
    request.onerror = () => reject(request.error);
    request.onsuccess = () => {
      const chunksToDelete = request.result.filter((c: ChunkInfo) => c.filePath === filePath);
      let deleted = 0;
      tx.oncomplete = () => resolve();
      tx.onerror = () => reject(tx.error);
      for (const chunk of chunksToDelete) {
        const delReq = store.delete(chunk.id);
        delReq.onsuccess = () => {
          deleted++;
          if (deleted === chunksToDelete.length) {
            tx.commit();
          }
        };
      }
      if (chunksToDelete.length === 0) {
        tx.commit();
      }
    };
    db.close();
  });
}

export async function reconstructFile(filePath: string): Promise<Blob | null> {
  const chunks = await getAllChunks(filePath);
  if (chunks.length === 0) return null;
  
  const totalChunks = chunks[0].totalChunks;
  if (chunks.length !== totalChunks) {
    throw new Error(`Missing chunks: got ${chunks.length}, expected ${totalChunks}`);
  }
  
  const buffers = chunks.map(c => c.data);
  return new Blob(buffers);
}

export async function reconstructFileStream(
  filePath: string,
  writable: FileSystemWritableFileStream,
  totalChunks: number,
  onProgress?: (loaded: number, total: number) => void
): Promise<void> {
  const db = await openDB();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(STORE_NAME, "readonly");
    const store = tx.objectStore(STORE_NAME);
    
    const pendingChunks: Map<number, ArrayBuffer> = new Map();
    let processed = 0;
    
    const writeNextInOrder = () => {
      while (pendingChunks.has(processed)) {
        const chunkData = pendingChunks.get(processed)!;
        pendingChunks.delete(processed);
        writable.write(chunkData).then(() => {
          processed++;
          onProgress?.(processed, totalChunks);
        }).catch(reject);
      }
    };
    
    const checkAndWrite = () => {
      writeNextInOrder();
      if (processed < totalChunks) {
        setTimeout(checkAndWrite, 10);
      } else {
        writable.close().then(resolve).catch(reject);
      }
    };
    
    const request = store.openCursor();
    request.onsuccess = () => {
      const cursor = request.result as IDBCursorWithValue | null;
      if (cursor) {
        const chunk = cursor.value as ChunkInfo;
        if (chunk.filePath === filePath) {
          pendingChunks.set(chunk.chunkIndex, chunk.data);
        }
        cursor.continue();
      } else {
        checkAndWrite();
      }
    };
    request.onerror = () => reject(request.error);
    db.close();
  });
}

export async function getChunkCount(filePath: string): Promise<number> {
  const chunks = await getAllChunks(filePath);
  return chunks.length;
}

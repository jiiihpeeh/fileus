import { encode, decode } from "@msgpack/msgpack";

const charset = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";

function generateRandomString(length: number, charset: string): string {
  const array = new Uint8Array(length);
  crypto.getRandomValues(array);
  return Array.from(array, (byte) => charset[byte % charset.length]).join("");
}

function randomLength(min: number, max: number): number {
  const array = new Uint8Array(1);
  crypto.getRandomValues(array);
  return min + (array[0] % (max - min + 1));
}

export function generateSalt(): string {
  return generateRandomString(randomLength(13, 98), charset);
}

export function generateNewKey(): string {
  return generateRandomString(32, charset);
}

async function compressGzip(data: Uint8Array): Promise<Uint8Array> {
  const stream = new Response(data).body!.pipeThrough(new CompressionStream('gzip'));
  return new Uint8Array(await new Response(stream).arrayBuffer());
}

async function decompressGzip(data: Uint8Array | ArrayBuffer): Promise<Uint8Array> {
  const bytes = data instanceof Uint8Array ? data : new Uint8Array(data);
  const stream = new Response(bytes).body!.pipeThrough(new DecompressionStream('gzip'));
  return new Uint8Array(await new Response(stream).arrayBuffer());
}

export async function encryptSession(
  newKey: string,
  sharedKey: string
): Promise<Uint8Array> {
  const salt = generateRandomString(64, charset);
  const keyData = new TextEncoder().encode(sharedKey);
  const hash = await crypto.subtle.digest("SHA-256", keyData);
  const key = await crypto.subtle.importKey("raw", hash, "AES-GCM", false, ["encrypt"]);

  const nonce = crypto.getRandomValues(new Uint8Array(12));
  // Session handshake does NOT use gzip
  const msgpack_data = encode([salt, newKey]);
  const ciphertext = await crypto.subtle.encrypt({ name: "AES-GCM", iv: nonce }, key, msgpack_data);

  const combined = new Uint8Array(nonce.length + ciphertext.byteLength);
  combined.set(nonce);
  combined.set(new Uint8Array(ciphertext), nonce.length);

  return combined;
}

/// Decrypt API message from raw bytes (no base64).
/// Input: nonce (12 bytes) || ciphertext
/// Output: inner payload bytes
export async function decryptApiMessage(
  encryptedData: Uint8Array,
  sessionKey: string
): Promise<Uint8Array> {
  if (encryptedData.length < 12 + 16) {
    throw new Error("Invalid encrypted data");
  }
  
  const nonce = encryptedData.slice(0, 12);
  const ciphertext = encryptedData.slice(12);

  const keyData = new TextEncoder().encode(sessionKey);
  const hash = await crypto.subtle.digest("SHA-256", keyData);
  const key = await crypto.subtle.importKey("raw", hash, "AES-GCM", false, ["decrypt"]);

  const decryptedBuffer = await crypto.subtle.decrypt(
    { name: "AES-GCM", iv: nonce },
    key,
    ciphertext
  );

  const decoded = decode(new Uint8Array(decryptedBuffer)) as any;

  console.debug("[CRYPTO] decoded:", decoded);
  
  let compressedPayload: any;
  if (Array.isArray(decoded)) {
    compressedPayload = decoded[1];
  } else {
    compressedPayload = decoded.payload || decoded;
  }
  
  // Ensure we have a Uint8Array for decompression
  const compressedBytes = compressedPayload instanceof Uint8Array 
    ? compressedPayload 
    : new Uint8Array(compressedPayload);

  // Decompress the payload
  const decompressed = await decompressGzip(compressedBytes);
  
  console.debug("[CRYPTO] decompressed bytes:", decompressed.length);
  return decompressed;
}

export async function decryptApiBinaryResponse(
  encryptedData: ArrayBuffer,
  sessionKey: string
): Promise<{ metadata: any; payload: Uint8Array }> {
  const bytes = new Uint8Array(encryptedData);
  
  const nonce = bytes.slice(0, 12);
  const ciphertext = bytes.slice(12);
  
  const keyData = new TextEncoder().encode(sessionKey);
  const hash = await crypto.subtle.digest("SHA-256", keyData);
  const key = await crypto.subtle.importKey("raw", hash, "AES-GCM", false, ["decrypt"]);
  
  const plaintext = await crypto.subtle.decrypt({ name: "AES-GCM", iv: nonce }, key, ciphertext);
  
  // Decompress the whole thing
  const decompressed = await decompressGzip(new Uint8Array(plaintext));
  
  const decoded = decode(decompressed) as any;
  if (Array.isArray(decoded)) {
    return {
      metadata: decoded[1],
      payload: new Uint8Array(decoded[2]),
    };
  }
  return {
    metadata: decoded.metadata,
    payload: new Uint8Array(decoded.payload),
  };
}

export async function decryptApiBinarySimple(
  encryptedData: ArrayBuffer,
  sessionKey: string
): Promise<Uint8Array> {
  const bytes = new Uint8Array(encryptedData);
  
  const nonce = bytes.slice(0, 12);
  const ciphertext = bytes.slice(12);
  
  const keyData = new TextEncoder().encode(sessionKey);
  const hash = await crypto.subtle.digest("SHA-256", keyData);
  const key = await crypto.subtle.importKey("raw", hash, "AES-GCM", false, ["decrypt"]);
  
  const plaintext = await crypto.subtle.decrypt({ name: "AES-GCM", iv: nonce }, key, ciphertext);
  
  // Decompress
  const decompressed = await decompressGzip(new Uint8Array(plaintext));
  
  return decompressed;
}

/// Encrypt API message to raw bytes (no base64).
/// Returns: nonce (12 bytes) || ciphertext
export async function encryptApiMessage(
  payload: Uint8Array,
  sessionKey: string
): Promise<Uint8Array> {
  const salt = generateRandomString(64, charset);
  const keyData = new TextEncoder().encode(sessionKey);
  const hash = await crypto.subtle.digest("SHA-256", keyData);
  const key = await crypto.subtle.importKey("raw", hash, "AES-GCM", false, ["encrypt"]);

  // Compress payload
  const compressedPayload = await compressGzip(payload);

  const nonce = crypto.getRandomValues(new Uint8Array(12));
  const msgpack_data = encode([salt, compressedPayload]);
  const ciphertext = await crypto.subtle.encrypt({ name: "AES-GCM", iv: nonce }, key, msgpack_data);

  const combined = new Uint8Array(nonce.length + ciphertext.byteLength);
  combined.set(nonce);
  combined.set(new Uint8Array(ciphertext), nonce.length);

  return combined;
}

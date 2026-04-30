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

export async function encryptSession(
  newKey: string,
  sharedKey: string
): Promise<Uint8Array> {
  const salt = generateRandomString(64, charset);
  const keyData = new TextEncoder().encode(sharedKey);
  const hash = await crypto.subtle.digest("SHA-256", keyData);
  const key = await crypto.subtle.importKey("raw", hash, "AES-GCM", false, ["encrypt"]);

  const nonce = crypto.getRandomValues(new Uint8Array(12));
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

  const decryptedBytes = new Uint8Array(decryptedBuffer);
  const decoded = decode(decryptedBytes) as any;

  console.debug("[CRYPTO] decoded:", decoded);
  
  let result: Uint8Array;
  if (Array.isArray(decoded)) {
    result = decoded[1];
  } else {
    result = decoded.payload || decoded;
  }
  
  console.debug("[CRYPTO] payload bytes:", result.length);
  return result;
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
  
  const decoded = decode(new Uint8Array(plaintext)) as any;
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
  
  return new Uint8Array(plaintext);
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

  const nonce = crypto.getRandomValues(new Uint8Array(12));
  const msgpack_data = encode([salt, payload]);
  const ciphertext = await crypto.subtle.encrypt({ name: "AES-GCM", iv: nonce }, key, msgpack_data);

  const combined = new Uint8Array(nonce.length + ciphertext.byteLength);
  combined.set(nonce);
  combined.set(new Uint8Array(ciphertext), nonce.length);

  return combined;
}

// Texture packs: players pick image files in Settings; any file whose name
// matches a game image (e.g. "tree_1.png", "hat_7.png") replaces it. Files are
// kept in IndexedDB so the pack survives reloads.

const DB = "roamer-texture-pack";
const STORE = "files";

/** Opens the database (resolves to null if IndexedDB isn't available). */
function openDb() {
  return new Promise((resolve) => {
    try {
      const req = indexedDB.open(DB, 1);
      req.onupgradeneeded = () => req.result.createObjectStore(STORE);
      req.onsuccess = () => resolve(req.result);
      req.onerror = () => resolve(null);
    } catch {
      resolve(null);
    }
  });
}

/** Runs one request against the store. */
async function withStore(mode, fn) {
  const db = await openDb();
  if (!db) return null;
  return new Promise((resolve) => {
    const tx = db.transaction(STORE, mode);
    const req = fn(tx.objectStore(STORE));
    tx.oncomplete = () => resolve(req?.result ?? null);
    tx.onerror = () => resolve(null);
  });
}

/** File name of an asset URL, without Vite's hash ("tree_1-abc12.png" -> "tree_1.png"). */
export function assetName(url) {
  const file = String(url).split("/").pop().split("?")[0];
  return file.replace(/-[A-Za-z0-9_]{6,}(\.[a-z0-9]+)$/i, "$1");
}

let overrides = new Map();

/** Loads the saved pack. Call once before loading textures. */
export async function loadTexturePack() {
  const entries = (await withStore("readonly", (s) => s.getAll())) || [];
  const keys = (await withStore("readonly", (s) => s.getAllKeys())) || [];
  overrides = new Map(keys.map((k, i) => [k, URL.createObjectURL(entries[i])]));
  return overrides.size;
}

/** The URL to load for an asset (the pack's version if there is one). */
export function resolveAsset(url) {
  return overrides.get(assetName(url)) || url;
}

/** Saves picked image files as the texture pack. */
export async function saveTexturePack(files) {
  await withStore("readwrite", (s) => s.clear());
  let count = 0;
  for (const file of files) {
    if (!file.type.startsWith("image/")) continue;
    await withStore("readwrite", (s) => s.put(file, file.name));
    count++;
  }
  return count;
}

/** Removes the texture pack. */
export async function clearTexturePack() {
  await withStore("readwrite", (s) => s.clear());
  overrides = new Map();
}

/** Number of images in the current pack. */
export function texturePackSize() {
  return overrides.size;
}

import JSON5 from "json5";

export interface ImportedThemeAsset {
  path: string;
  bytes: Uint8Array;
}

export interface ImportedThemeMetadata {
  themeId: string;
  version?: string;
  displayName?: string;
  authorName?: string;
  description?: string;
  themeLicenseText?: string;
}

export interface ImportedThemeBundle {
  kind: "toolpkg" | "json";
  themeJson: string;
  metadata?: ImportedThemeMetadata;
  assets: ImportedThemeAsset[];
  source?: ImportedToolPkgSource;
}

export interface ImportedToolPkgSource {
  bytes: Uint8Array;
  manifest: Record<string, unknown>;
  manifestPath: string;
  entries: Map<string, Uint8Array>;
  template: ToolPkgThemeDeclaration;
}

export interface ToolPkgThemeDeclaration {
  id: string;
  path: string;
  preview: string;
}

export type SelectToolPkgTheme = (themes: readonly ToolPkgThemeDeclaration[]) => Promise<string>;

interface ZipEntry {
  name: string;
  compressionMethod: number;
  compressedBytes: Uint8Array;
  uncompressedSize: number;
}

/** Reads ZIP entry offsets without changing the archive paths. */
function readZipEntries(archive: Uint8Array): ZipEntry[] {
  const view = new DataView(archive.buffer, archive.byteOffset, archive.byteLength);
  let eocdOffset = -1;
  const minEocdSearch = Math.max(0, archive.byteLength - 65557);
  for (let i = archive.byteLength - 22; i >= minEocdSearch; i -= 1) {
    if (view.getUint32(i, true) === 0x06054b50) {
      eocdOffset = i;
      break;
    }
  }
  if (eocdOffset < 0) {
    throw new Error("不是有效的 .toolpkg / ZIP 归档文件（缺少 EOCD 签名）");
  }

  const totalEntries = view.getUint16(eocdOffset + 10, true);
  const centralDirOffset = view.getUint32(eocdOffset + 16, true);
  const decoder = new TextDecoder("utf-8");
  const entries: ZipEntry[] = [];

  let ptr = centralDirOffset;
  for (let idx = 0; idx < totalEntries; idx += 1) {
    if (ptr + 46 > archive.byteLength || view.getUint32(ptr, true) !== 0x02014b50) {
      throw new Error("ZIP 中央目录损坏");
    }
    const compressionMethod = view.getUint16(ptr + 10, true);
    const compressedSize = view.getUint32(ptr + 20, true);
    const uncompressedSize = view.getUint32(ptr + 24, true);
    const nameLen = view.getUint16(ptr + 28, true);
    const extraLen = view.getUint16(ptr + 30, true);
    const commentLen = view.getUint16(ptr + 32, true);
    const localHeaderOffset = view.getUint32(ptr + 42, true);

    const nameBytes = archive.subarray(ptr + 46, ptr + 46 + nameLen);
    const name = decoder.decode(nameBytes);
    ptr += 46 + nameLen + extraLen + commentLen;

    if (name.endsWith("/")) continue;

    if (
      localHeaderOffset + 30 > archive.byteLength ||
      view.getUint32(localHeaderOffset, true) !== 0x04034b50
    ) {
      throw new Error(`ZIP 本地文件头损坏: ${name}`);
    }
    const localNameLen = view.getUint16(localHeaderOffset + 26, true);
    const localExtraLen = view.getUint16(localHeaderOffset + 28, true);
    const dataStart = localHeaderOffset + 30 + localNameLen + localExtraLen;
    const dataEnd = dataStart + compressedSize;
    if (dataEnd > archive.byteLength) {
      throw new Error(`ZIP 条目超出归档边界: ${name}`);
    }
    entries.push({
      name,
      compressionMethod,
      compressedBytes: archive.subarray(dataStart, dataEnd),
      uncompressedSize,
    });
  }
  return entries;
}

/** Expands a supported archive entry under the declared byte budget. */
async function decompressEntry(entry: ZipEntry): Promise<Uint8Array> {
  if (entry.uncompressedSize > 32 * 1024 * 1024) throw new Error("ToolPkg entry exceeds its byte budget");
  if (entry.compressionMethod === 0) {
    if (entry.compressedBytes.byteLength !== entry.uncompressedSize) throw new Error("ToolPkg entry size mismatch");
    return new Uint8Array(entry.compressedBytes);
  }
  if (entry.compressionMethod !== 8) throw new Error("Unsupported ToolPkg compression method");
  const stream = new Blob([Uint8Array.from(entry.compressedBytes).buffer]).stream()
    .pipeThrough(new DecompressionStream("deflate-raw"));
  const bytes = new Uint8Array(await new Response(stream).arrayBuffer());
  if (bytes.byteLength !== entry.uncompressedSize) throw new Error("ToolPkg entry size mismatch");
  return bytes;
}

/** Reads a plain or Chinese-localized field for this Chinese-language authoring form. */
function editableManifestText(value: unknown): string | undefined {
  if (value === undefined) return undefined;
  if (typeof value === "string") return value;
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw new Error("ToolPkg 本地化文本必须是字符串或语言映射");
  }
  const object = value as Record<string, unknown>;
  const locales = Object.hasOwn(object, "values") ? object.values : object;
  if (locales === null || typeof locales !== "object" || Array.isArray(locales)) {
    throw new Error("ToolPkg 本地化文本的 values 必须是语言映射");
  }
  const zh = (locales as Record<string, unknown>).zh;
  if (zh !== undefined && typeof zh !== "string") throw new Error("ToolPkg zh 文本必须是字符串");
  return zh;
}

/** Resolves the same manifest names and ordering as the shared ToolPkg parser. */
function manifestEntry(files: Map<string, Uint8Array>): string {
  const candidates = [...files.keys()].map(path => {
    const lower = path.toLowerCase();
    const filename = lower.slice(lower.lastIndexOf("/") + 1);
    const rank = lower === "manifest.hjson" ? 0 : lower === "manifest.json" ? 1 :
      filename === "manifest.hjson" ? 2 : filename === "manifest.json" ? 3 : 4;
    return { path, rank };
  }).filter(candidate => candidate.rank < 4)
    .sort((left, right) => left.rank - right.rank || (left.path < right.path ? -1 : left.path > right.path ? 1 : 0));
  if (candidates.length === 0) throw new Error("ToolPkg 缺少 manifest.json 或 manifest.hjson");
  return candidates[0].path;
}

/** Reads visual JSON or a declarative theme from the standard ToolPkg manifest. */
export async function parseImportedThemeFile(
  file: File,
  selectTheme?: SelectToolPkgTheme,
): Promise<ImportedThemeBundle> {
  if (file.size === 0 || file.size > 64 * 1024 * 1024) throw new Error("ToolPkg file exceeds its byte budget");
  const bytes = new Uint8Array(await file.arrayBuffer());
  const decoder = new TextDecoder("utf-8", { fatal: true });
  if (file.name.toLowerCase().endsWith(".json")) {
    const theme = JSON.parse(decoder.decode(bytes));
    if (theme === null || typeof theme !== "object" || Array.isArray(theme)) throw new Error("主题 JSON 必须是对象");
    return { kind: "json", themeJson: JSON.stringify(theme, null, 2), assets: [] };
  }
  if (!file.name.toLowerCase().endsWith(".toolpkg")) throw new Error("请选择 .toolpkg 或主题 JSON 文件");
  const entries = readZipEntries(bytes);
  const files = new Map<string, Uint8Array>();
  let total = 0;
  for (const entry of entries) {
    if (files.has(entry.name)) throw new Error("ToolPkg contains duplicate entries");
    if (entry.name.split("/").some(part => part === "" || part === "." || part === "..") || entry.name.includes("\\")) {
      throw new Error("ToolPkg contains an unsafe path");
    }
    total += entry.uncompressedSize;
    if (total > 128 * 1024 * 1024) throw new Error("Expanded ToolPkg exceeds its byte budget");
    files.set(entry.name, await decompressEntry(entry));
  }
  const manifestPath = manifestEntry(files);
  const manifestBytes = files.get(manifestPath)!;
  const text = decoder.decode(manifestBytes);
  const manifest = manifestPath.toLowerCase().endsWith(".hjson") ?
    JSON5.parse(text.split("\n").filter(line => !line.trimStart().startsWith("//")).join("\n")) : JSON.parse(text);
  const manifestBase = manifestPath.slice(0, manifestPath.lastIndexOf("/") + 1);
  if (manifest == null || typeof manifest !== "object" || Array.isArray(manifest)) {
    throw new Error("ToolPkg manifest 必须是对象");
  }
  if (!Array.isArray(manifest.themes) || manifest.themes.length === 0) {
    throw new Error("此 ToolPkg 没有可供编辑的 theme 声明");
  }
  const ids = new Set<string>();
  for (const declaration of manifest.themes) {
    if (declaration == null || typeof declaration !== "object" ||
        typeof declaration.id !== "string" || !declaration.id.trim() || declaration.id.includes(":") || ids.has(declaration.id)) {
      throw new Error("ToolPkg themes 声明的 ID 必须非空且唯一");
    }
    ids.add(declaration.id);
    for (const path of [declaration.path, declaration.preview]) {
      if (typeof path !== "string" || !files.has(`${manifestBase}${path}`)) {
        throw new Error(`ToolPkg theme 声明引用的文件不存在: ${String(path)}`);
      }
    }
  }
  const declarations: ToolPkgThemeDeclaration[] = manifest.themes;
  let theme: ToolPkgThemeDeclaration;
  if (declarations.length === 1) {
    theme = declarations[0];
  } else {
    if (selectTheme == null) throw new Error("请指定此 ToolPkg 中要编辑的 theme 声明");
    const selectedId = await selectTheme(declarations);
    const selected = declarations.find(declaration => declaration.id === selectedId);
    if (selected == null) throw new Error(`ToolPkg 中不存在 theme 声明: ${selectedId}`);
    theme = selected;
  }
  if (typeof manifest.toolpkg_id !== "string" || !manifest.toolpkg_id) {
    throw new Error("ToolPkg manifest.toolpkg_id is required");
  }
  if (manifest.version !== undefined && typeof manifest.version !== "string") {
    throw new Error("ToolPkg manifest.version must be text");
  }
  const metadata: ImportedThemeMetadata = {
    themeId: manifest.toolpkg_id,
    version: manifest.version,
    displayName: editableManifestText(manifest.display_name),
    description: editableManifestText(manifest.description),
  };
  if (manifest.author !== undefined) {
    if (typeof manifest.author === "string") {
      metadata.authorName = manifest.author;
    } else if (Array.isArray(manifest.author) && manifest.author.every((author: unknown) => typeof author === "string")) {
      metadata.authorName = manifest.author.join(", ");
    } else {
      throw new Error("ToolPkg manifest.author must be text or a list of text");
    }
  }
  if (manifest.license !== undefined && manifest.license !== null) {
    if (typeof manifest.license !== "string") throw new Error("ToolPkg manifest.license must be a resource path");
    const licenseBytes = files.get(`${manifestBase}${manifest.license}`);
    if (licenseBytes == null) throw new Error("ToolPkg 引用的协议文本不存在");
    metadata.themeLicenseText = decoder.decode(licenseBytes);
  }
  const themeBytes = files.get(`${manifestBase}${theme.path}`)!;
  const visual = JSON.parse(decoder.decode(themeBytes));
  if (visual === null || typeof visual !== "object" || Array.isArray(visual)) {
    throw new Error("主题 JSON 必须是对象");
  }
  const base = `${manifestBase}${theme.path.slice(0, theme.path.lastIndexOf("/") + 1)}`;
  const resourceDeclarations = (theme as ToolPkgThemeDeclaration & { files?: {path: string}[] }).files;
  const assets: ImportedThemeAsset[] = [];
  if (resourceDeclarations != null) {
    for (const resource of resourceDeclarations) {
      if (!resource.path.startsWith("assets/")) continue;
      const bytes = files.get(`${base}${resource.path}`);
      if (bytes == null) throw new Error(`ToolPkg 模板资源不存在: ${resource.path}`);
      assets.push({ path: resource.path, bytes });
    }
  }
  return { kind: "toolpkg", themeJson: JSON.stringify(visual, null, 2), metadata, assets,
    source: { bytes, manifest, manifestPath, entries: files, template: theme } };
}

/** Calculates the ZIP CRC for one stored archive entry. */
function zipCrc32(bytes: Uint8Array): number {
  let crc = 0xffffffff;
  for (const byte of bytes) {
    crc ^= byte;
    for (let bit = 0; bit < 8; bit++) crc = (crc >>> 1) ^ ((crc & 1) * 0xedb88320);
  }
  return (crc ^ 0xffffffff) >>> 0;
}

/** Writes ordinary ToolPkg entries without interpreting other package capabilities. */
function writeStoredToolPkg(entries: Map<string, Uint8Array>): Uint8Array {
  const encoder = new TextEncoder();
  const locals: Uint8Array[] = [];
  const directory: Uint8Array[] = [];
  let offset = 0;
  for (const [path, bytes] of entries) {
    const name = encoder.encode(path);
    const local = new Uint8Array(30 + name.length + bytes.length);
    const lv = new DataView(local.buffer);
    lv.setUint32(0, 0x04034b50, true); lv.setUint16(4, 20, true);
    lv.setUint16(6, 0x0800, true); lv.setUint32(14, zipCrc32(bytes), true);
    lv.setUint32(18, bytes.length, true); lv.setUint32(22, bytes.length, true);
    lv.setUint16(26, name.length, true); local.set(name, 30); local.set(bytes, 30 + name.length);
    const central = new Uint8Array(46 + name.length);
    const cv = new DataView(central.buffer);
    cv.setUint32(0, 0x02014b50, true); cv.setUint16(4, 20, true); cv.setUint16(6, 20, true);
    cv.setUint16(8, 0x0800, true); cv.setUint32(16, zipCrc32(bytes), true);
    cv.setUint32(20, bytes.length, true); cv.setUint32(24, bytes.length, true);
    cv.setUint16(28, name.length, true); cv.setUint32(42, offset, true); central.set(name, 46);
    locals.push(local); directory.push(central); offset += local.length;
  }
  const directorySize = directory.reduce((sum, entry) => sum + entry.length, 0);
  const end = new Uint8Array(22); const ev = new DataView(end.buffer);
  ev.setUint32(0, 0x06054b50, true); ev.setUint16(8, entries.size, true); ev.setUint16(10, entries.size, true);
  ev.setUint32(12, directorySize, true); ev.setUint32(16, offset, true);
  const result = new Uint8Array(offset + directorySize + end.length);
  let cursor = 0;
  for (const part of [...locals, ...directory, end]) { result.set(part, cursor); cursor += part.length; }
  return result;
}

/** Replaces the selected template while retaining all other capabilities and package entries. */
export async function updateToolPkgTheme(source: ImportedToolPkgSource, authored: Uint8Array): Promise<Uint8Array> {
  const edited = await parseImportedThemeFile(new File([Uint8Array.from(authored).buffer], "edited.toolpkg"));
  if (edited.source == null) throw new Error("Template export must produce ToolPkg entries");
  const manifest = structuredClone(source.manifest);
  const entries = new Map(source.entries);
  const templates = manifest.themes as ToolPkgThemeDeclaration[];
  const index = templates.findIndex(template => template.id === source.template.id);
  if (index < 0) throw new Error("Selected template no longer exists in ToolPkg");
  const declaration = structuredClone(edited.source.template);
  declaration.id = source.template.id;
  declaration.path = source.template.path;
  declaration.preview = source.template.preview;
  templates[index] = declaration;
  for (const key of ["toolpkg_id", "version", "display_name", "author", "description"]) {
    if (Object.hasOwn(edited.source.manifest, key)) manifest[key] = edited.source.manifest[key];
    else delete manifest[key];
  }
  const manifestBase = source.manifestPath.slice(0, source.manifestPath.lastIndexOf("/") + 1);
  const templateBase = declaration.path.slice(0, declaration.path.lastIndexOf("/") + 1);
  const base = `${manifestBase}${templateBase}`;
  for (const [path, bytes] of edited.source.entries) {
    if (path === "manifest.json") continue;
    const target = path === "theme.json" ? `${manifestBase}${declaration.path}` : path === "preview.png" ? `${manifestBase}${declaration.preview}` : `${base}${path}`;
    entries.set(target, bytes);
  }
  if (typeof edited.source.manifest.license === "string") {
    manifest.license = `${templateBase}${edited.source.manifest.license}`;
  } else delete manifest.license;
  // Installation seals authenticate the original installed bytes and are not authoring metadata.
  entries.delete(".operit/market-install.seal");
  delete manifest.market_origin;
  entries.set(source.manifestPath, new TextEncoder().encode(JSON.stringify(manifest)));
  return writeStoredToolPkg(entries);
}

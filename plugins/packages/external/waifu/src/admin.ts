import type { Settings } from "./api.js";
import { readSettings, writeSettings } from "./settings.js";

export type AdminRequest = { action: "get"; profile: string } | { action: "set"; profile: string; settings: Settings };

/** Routes authoring commands through the provider's single main runtime. */
export function admin(request: AdminRequest): Promise<Settings> {
  return request.action === "get" ? readSettings(request.profile) : writeSettings(request.profile, request.settings);
}

/** Calls the package-private settings channel from tools and native Compose screens. */
export function callAdmin(request: AdminRequest): Promise<Settings> {
  return ToolPkg.ipc.call<AdminRequest, Settings>("waifu.settings", request, { targetRuntime: "main" });
}

/* METADATA
{
  "name": "waifu",
  "display_name": {
    "zh": "Waifu 模式",
    "en": "Waifu Mode"
  },
  "description": {
    "zh": "管理独立的流式分句与表情配置。",
    "en": "Manage waifu sentence and sticker profiles."
  },
  "category": "Chat",
  "tools": [
    {
      "name": "waifu_settings_get",
      "description": {
        "zh": "读取 Waifu 命名配置。",
        "en": "Read a named waifu profile."
      },
      "parameters": [
        {
          "name": "profile",
          "type": "string",
          "required": false,
          "description": {
            "zh": "配置名称，默认 default",
            "en": "Profile name, default: default"
          }
        }
      ]
    },
    {
      "name": "waifu_settings_set",
      "description": {
        "zh": "保存完整 Waifu 配置。settings_json 为包含打字延迟、表情、自拍及提示词的完整 JSON。",
        "en": "Save a complete waifu profile."
      },
      "parameters": [
        {
          "name": "profile",
          "type": "string",
          "required": true,
          "description": {
            "zh": "配置名称",
            "en": "Profile name"
          }
        },
        {
          "name": "settings_json",
          "type": "string",
          "required": true,
          "description": {
            "zh": "完整 Settings JSON",
            "en": "Complete Settings JSON"
          }
        }
      ]
    }
  ],
  "enabled_by_default": true
}
*/
import { callAdmin } from "./admin.js";
import type { Settings } from "./api.js";

/** Reads a named profile from the provider's authoritative runtime. */
export function waifu_settings_get(params: { profile?: string } = {}): Promise<Settings> {
  const { profile = "default" } = params;
  return callAdmin({ action: "get", profile });
}

/** Parses and saves one explicitly supplied complete profile. */
export function waifu_settings_set(params: { profile: string; settings_json: string }): Promise<Settings> {
  return callAdmin({ action: "set", profile: params.profile, settings: JSON.parse(params.settings_json) });
}

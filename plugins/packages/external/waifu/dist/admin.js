"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.admin = admin;
exports.callAdmin = callAdmin;
const settings_js_1 = require("./settings.js");
/** Routes authoring commands through the provider's single main runtime. */
function admin(request) {
    return request.action === "get" ? (0, settings_js_1.readSettings)(request.profile) : (0, settings_js_1.writeSettings)(request.profile, request.settings);
}
/** Calls the package-private settings channel from tools and native Compose screens. */
function callAdmin(request) {
    return ToolPkg.ipc.call("waifu.settings", request, { targetRuntime: "main" });
}

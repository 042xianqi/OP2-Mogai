"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.default = screen;
const admin_js_1 = require("../admin.js");
/** Renders profile-scoped sentence delivery, emotions and selfie prompt settings. */
function screen(ctx) {
    const [profile, setProfile] = ctx.useState("profile", "default");
    const [settings, setSettings] = ctx.useState("settings", null);
    const [busy, setBusy] = ctx.useState("busy", false);
    const [error, setError] = ctx.useState("error", "");
    const [message, setMessage] = ctx.useState("message", "");
    const [delay, setDelay] = ctx.useState("delay", "250");
    const [punctuation, setPunctuation] = ctx.useState("punctuation", false);
    const [emoticons, setEmoticons] = ctx.useState("emoticons", false);
    const [selfie, setSelfie] = ctx.useState("selfie", false);
    const [prompt, setPrompt] = ctx.useState("prompt", "");
    const [appearance, setAppearance] = ctx.useState("appearance", "");
    const [emojis, setEmojis] = ctx.useState("emojis", "{}");
    /** Reflects the exact profile returned by the authoritative provider runtime. */
    function sync(value) {
        setSettings(value);
        setDelay(String(value.charDelayMs));
        setPunctuation(value.removePunctuation);
        setEmoticons(value.enableEmoticons);
        setSelfie(value.enableSelfie);
        setPrompt(value.customPrompt);
        setAppearance(value.selfiePrompt);
        setEmojis(JSON.stringify(value.emojis, null, 2));
    }
    /** Loads or commits a named profile and surfaces the original validation error. */
    async function run(save) {
        if (busy)
            return;
        setBusy(true);
        setError("");
        setMessage("");
        try {
            if (save) {
                const value = { charDelayMs: Number(delay), removePunctuation: punctuation, enableEmoticons: emoticons, enableSelfie: selfie, customPrompt: prompt, selfiePrompt: appearance, emojis: JSON.parse(emojis) };
                sync(await (0, admin_js_1.callAdmin)({ action: "set", profile, settings: value }));
                setMessage("已保存，下一次回复生效。");
            }
            else
                sync(await (0, admin_js_1.callAdmin)({ action: "get", profile }));
        }
        catch (failure) {
            setError(failure instanceof Error ? failure.message : String(failure));
        }
        finally {
            setBusy(false);
        }
    }
    /** Creates one editable native text field. */
    function field(label, value, onValueChange, singleLine = true) {
        return ctx.UI.TextField({ label, value, onValueChange, singleLine, enabled: !busy, fillMaxWidth: true });
    }
    /** Creates a native boolean control beside its visible description. */
    function toggle(label, checked, onCheckedChange) {
        return ctx.UI.Row({ horizontalArrangement: "spaceBetween", fillMaxWidth: true }, [ctx.UI.Text({ text: label }), ctx.UI.Switch({ checked, onCheckedChange, enabled: !busy })]);
    }
    return ctx.UI.LazyColumn({ fillMaxSize: true, padding: 16, spacing: 12, onLoad: () => run(false) }, [
        ctx.UI.Text({ text: "Waifu 模式", style: "headlineMedium" }),
        ctx.UI.Text({ text: "供 QQBot 等依赖插件使用的流式分句和表情服务。配置以名称保存，可为不同角色创建独立配置。" }),
        field("配置名称", profile, setProfile), ctx.UI.Button({ text: "读取配置", enabled: !busy, onClick: () => run(false) }),
        field("每字符打字延迟（毫秒，首句立即发送，单句最多 3 秒）", delay, setDelay),
        toggle("移除句末标点", punctuation, setPunctuation), toggle("启用情绪表情图片", emoticons, setEmoticons),
        field("对话提示规则", prompt, setPrompt, false), toggle("启用自拍提示规则", selfie, setSelfie),
        field("自拍外貌关键词（使用已安装的绘图工具）", appearance, setAppearance, false),
        field("表情分类与图片 JSON（resource: 为内置图片，也可填写图片地址或文件路径）", emojis, setEmojis, false),
        ctx.UI.Button({ text: "保存配置", enabled: !busy && settings !== null, onClick: () => run(true) }),
        ctx.UI.Text({ text: error, color: "error" }), ctx.UI.Text({ text: message }),
    ]);
}

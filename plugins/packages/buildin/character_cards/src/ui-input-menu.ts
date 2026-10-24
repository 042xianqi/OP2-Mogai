import type { ComposeDslContext, ComposeNode } from "../../../../types/compose-dsl";
import type { JsonValue } from "./model";
import { readSelectorData, loadSelectorAvatars, type SelectorData, type SelectorOption } from "./ui-selector";
import { chatIdentity } from "./ui-contributions";
import { chatUiRoutes } from "./definition";

/** Keeps the plugin's authoritative selection and avatar resource inside its own DSL runtime. */
interface MenuState { data: SelectorData | null; unboundAvatar: string | null; error: string }

/** Finds the selected persisted record without replacing a broken binding with another character. */
function selectedOption(data: SelectorData): SelectorOption | null {
  if (data.selected === null) return null;
  const matching = data.options.filter(
    /** Matches the exact opaque selection confirmed by the existing selector reader. */
    option => option.token === data.selected,
  );
  if (matching.length !== 1) throw new Error("Current character must identify exactly one selector record");
  return matching[0];
}

/** Reproduces the original circular 32-pixel avatar using actual resource or stored image data. */
function menuAvatar(ctx: ComposeDslContext, option: SelectorOption | null, unboundAvatar: string | null): ComposeNode {
  const props = { width: 32, height: 32, contentScale: "crop" as const, modifier: ctx.Modifier.clip({ type: "circle" }) };
  if (option === null) {
    if (unboundAvatar === null) return ctx.UI.Box({ width: 32, height: 32, contentAlignment: "center" }, ctx.UI.Icon({ name: "Person", size: 24, tint: ctx.MaterialTheme.colorScheme.onSurfaceVariant }));
    return ctx.UI.Image({ ...props, path: unboundAvatar, contentDescription: "未绑定" });
  }
  switch (option.avatar.type) {
    case "uri": return ctx.UI.Image({ ...props, uri: option.avatar.uri, contentDescription: option.title });
    case "resource": return option.avatar.path === null
      ? ctx.UI.Box({ width: 32, height: 32, contentAlignment: "center" }, ctx.UI.Icon({ name: "Person", size: 24, tint: ctx.MaterialTheme.colorScheme.onSurfaceVariant }))
      : ctx.UI.Image({ ...props, path: option.avatar.path, contentDescription: option.title });
    case "model": return ctx.UI.Box({ width: 32, height: 32, contentAlignment: "center" }, ctx.UI.Icon({ name: "SmartToy", size: 24, tint: ctx.MaterialTheme.colorScheme.onSurfaceVariant }));
    case "group": return ctx.UI.Box({ width: 32, height: 32, contentAlignment: "center" }, ctx.UI.Icon({ name: "Groups", size: 24, tint: ctx.MaterialTheme.colorScheme.onSurfaceVariant }));
  }
}

/** Opens the package-owned execution control route for the currently bound group only. */
function groupExecutionRow(ctx: ComposeDslContext, chatId: string): ComposeNode {
  const colors = ctx.MaterialTheme.colorScheme;
  return ctx.UI.Row({ key: "group-execution", fillMaxWidth: true, paddingHorizontal: 12, paddingVertical: 6, verticalAlignment: "center",
    modifier: ctx.Modifier.heightIn({ minHeight: 48 }),
    /** Presents the registered control screen with the exact bound chat, never another chat. */
    onClick: (): JsonValue => ({ type: "toolpkg.ui.present", routeId: chatUiRoutes.execution,
      input: { mode: "group-execution", chatId } }),
  }, [
    ctx.UI.Box({ width: 32, height: 32, contentAlignment: "center" }, ctx.UI.Icon({ name: "PlayArrow", size: 24, tint: colors.onSurfaceVariant })),
    ctx.UI.Spacer({ width: 10 }),
    ctx.UI.Column({ weight: 1 }, [
      ctx.UI.Text({ text: "群组执行", style: "labelSmall", color: colors.onSurfaceVariant }),
      ctx.UI.Text({ text: "打开群组执行控制", style: "bodySmall", color: colors.onSurface, fontWeight: "600", maxLines: 1, overflow: "ellipsis" }),
    ]),
    ctx.UI.Icon({ name: "ChevronRight", size: 20, tint: colors.onSurfaceVariant }),
  ]);
}

/** Loads and draws the sole menu row in TypeScript, delegating only generic route presentation to the host. */
export function renderChatInputMenu(ctx: ComposeDslContext): ComposeNode {
  const [chatId] = ctx.useState<string | null>("chatId", null);
  if (chatId !== null) chatIdentity(chatId, "input menu chatId");
  const owner = ctx.useRef("input-menu-chat", chatId);
  const [state, setState] = ctx.useState<MenuState>("input-menu-state", { data: null, unboundAvatar: null, error: "" });
  /** Rejects callbacks retained from a different chat context before reading or opening a selector. */
  function assertOwner(): void {
    const [currentChatId] = ctx.useState<string | null>("chatId", null);
    if (currentChatId !== owner.current) throw new Error("Input menu chat owner changed");
  }
  assertOwner();
  /** Reads the same main-runtime records and explicit chat binding used by the native selector. */
  async function load(): Promise<void> {
    assertOwner();
    try {
      const data = await readSelectorData({ mode: "select", chatId, kind: "all", selected: null });
      const option = selectedOption(data);
      assertOwner(); setState({ data, unboundAvatar: null, error: "" });
      // The menu label and action are available before any default image lookup.
      void (async () => {
        const resolved = await loadSelectorAvatars(data);
        const defaultOption = resolved.options.find(candidate => candidate.avatar.type === "resource");
        const unboundAvatar = option === null
          ? defaultOption?.avatar.type === "resource" && defaultOption.avatar.path !== null
            ? defaultOption.avatar.path : await ToolPkg.readResource("character_default_avatar", "operit-avatar.png")
          : null;
        if (unboundAvatar !== null && unboundAvatar.trim() === "") return;
        assertOwner(); setState({ data: resolved, unboundAvatar, error: "" });
      })().catch(() => {});
    } catch (error) {
      assertOwner(); setState({ data: null, unboundAvatar: null, error: String(error) });
    }
  }
  const colors = ctx.MaterialTheme.colorScheme;
  if (state.error !== "") return ctx.UI.Text({ text: state.error, color: colors.error, paddingHorizontal: 12, paddingVertical: 6 });
  if (state.data === null) return ctx.UI.Box({ key: "current-character-loading", onLoad: load, modifier: ctx.Modifier.heightIn({ minHeight: 48 }), fillMaxWidth: true, contentAlignment: "center" }, ctx.UI.CircularProgressIndicator({ width: 18, height: 18 }));
  const option = selectedOption(state.data);
  /** Reports the actual binding category so the selector lists groups for group-bound chats. */
  const boundKind = option !== null && option.kind === "group" ? "group" : "all";
  const characterRow = ctx.UI.Row({ key: "current-character", fillMaxWidth: true, paddingHorizontal: 12, paddingVertical: 6, verticalAlignment: "center",
    modifier: ctx.Modifier.heightIn({ minHeight: 48 }),
    /** Opens the registered TS selector while the host carries its real chat lifecycle guard. */
    onClick: (): JsonValue => {
      assertOwner();
      return { type: "toolpkg.ui.present", routeId: chatUiRoutes.selection,
        input: { mode: "select", chatId, kind: boundKind, selected: option !== null && option.kind !== "group" ? option.selection : null } };
    },
  }, [
    menuAvatar(ctx, option, state.unboundAvatar), ctx.UI.Spacer({ width: 10 }),
    ctx.UI.Column({ weight: 1 }, [
      ctx.UI.Text({ text: "当前角色卡", style: "labelSmall", color: colors.onSurfaceVariant }),
      ctx.UI.Text({ text: option === null ? "未绑定" : option.title, style: "bodySmall", color: colors.onSurface, fontWeight: "600", maxLines: 1, overflow: "ellipsis" }),
    ]),
    ctx.UI.Icon({ name: "ChevronRight", size: 20, tint: colors.onSurfaceVariant }),
  ]);
  /** Keeps the execution control reachable only for a chat actually bound to one group. */
  if (boundKind !== "group" || chatId === null) return characterRow;
  return ctx.UI.Column({ key: "current-character-menu", fillMaxWidth: true }, [characterRow, groupExecutionRow(ctx, chatId)]);
}

import { getService } from "./service-runtime";
import { requireId } from "./validation";

/** Applies the opened conversation's bound theme through the shared service without changing role selection. */
export async function onChatViewTheme(event: ToolPkg.ChatViewHookEvent): Promise<void> {
  if (event.eventName !== "view_opened") return;
  const chatId = requireId(event.eventPayload.chatId, "opened chat id");
  await (await getService()).applyChatTheme(chatId);
}

/** Registers the existing generic navigation event without reading settings or opening plugin storage. */
export function registerChatViewTheme(): void {
  ToolPkg.registerChatViewHook({ id: "character-bound-theme", function: onChatViewTheme });
}

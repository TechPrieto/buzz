import type { Channel } from "@/shared/api/types";

export function isHuddleBackingChannel(channel: Channel): boolean {
  if (channel.ttlSeconds !== 3_600) return false;
  const name = channel.name.trim().toLowerCase();
  return (
    name === "huddle" ||
    name.endsWith(" huddle") ||
    /^huddle-[0-9a-f]{8}$/.test(name)
  );
}

export function shouldShowSidebarChannel(
  channel: Channel,
  revealedHuddleChannelIds: ReadonlySet<string>,
): boolean {
  return (
    !isHuddleBackingChannel(channel) || revealedHuddleChannelIds.has(channel.id)
  );
}

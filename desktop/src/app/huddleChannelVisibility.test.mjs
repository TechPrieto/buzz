import assert from "node:assert/strict";
import test from "node:test";

import {
  isHuddleBackingChannel,
  shouldShowSidebarChannel,
} from "./huddleChannelVisibility.ts";

function channel(overrides = {}) {
  return {
    id: "channel-id",
    name: "general",
    ttlSeconds: null,
    ...overrides,
  };
}

test("ordinary channels stay visible without an explicit reveal", () => {
  assert.equal(shouldShowSidebarChannel(channel(), new Set()), true);
});

test("huddle backing channels stay hidden by default", () => {
  const staleHuddle = channel({
    id: "stale-huddle",
    name: "general huddle",
    ttlSeconds: 3_600,
  });

  assert.equal(isHuddleBackingChannel(staleHuddle), true);
  assert.equal(shouldShowSidebarChannel(staleHuddle, new Set()), false);
});

test("an explicitly revealed huddle channel appears in the sidebar", () => {
  const huddle = channel({
    id: "active-huddle",
    name: "huddle",
    ttlSeconds: 3_600,
  });

  assert.equal(shouldShowSidebarChannel(huddle, new Set([huddle.id])), true);
});

test("huddle-shaped names without the huddle TTL remain ordinary channels", () => {
  const ordinaryChannel = channel({ name: "design huddle" });

  assert.equal(isHuddleBackingChannel(ordinaryChannel), false);
  assert.equal(shouldShowSidebarChannel(ordinaryChannel, new Set()), true);
});

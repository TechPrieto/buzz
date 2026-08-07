import assert from "node:assert/strict";
import test from "node:test";

import {
  buildNamedThreadTags,
  collectThreadAgentAudience,
  resolveThreadTitle,
} from "./threadConversation.ts";

test("buildNamedThreadTags emits a normalized subject only for non-empty titles", () => {
  assert.deepEqual(buildNamedThreadTags("  Plan agosto   — contenido  "), [
    ["subject", "Plan agosto — contenido"],
  ]);
  assert.deepEqual(buildNamedThreadTags("   "), []);
});

test("resolveThreadTitle prefers an explicit subject tag", () => {
  assert.equal(
    resolveThreadTitle("Opening message", [["subject", "Plan agosto"]]),
    "Plan agosto",
  );
});

test("resolveThreadTitle falls back to the first non-empty content line", () => {
  assert.equal(
    resolveThreadTitle("\n  # Lanzamiento Q3  \nMore detail", []),
    "Lanzamiento Q3",
  );
});

test("resolveThreadTitle truncates long fallback titles", () => {
  const title = resolveThreadTitle("x".repeat(120), []);
  assert.equal(title.length, 80);
  assert.equal(title.endsWith("…"), true);
});

test("collectThreadAgentAudience restores agents addressed anywhere in the thread", () => {
  const owner = "1".repeat(64);
  const agent = "a".repeat(64);
  const other = "b".repeat(64);
  const messages = [
    { signerPubkey: owner, tags: [["h", "channel"]] },
    {
      signerPubkey: owner,
      tags: [
        ["p", other],
        ["p", agent],
      ],
    },
    { signerPubkey: other, tags: [["p", agent]] },
  ];

  assert.deepEqual(
    collectThreadAgentAudience(messages, owner, (pubkey) => pubkey === agent),
    [agent],
  );
});

import assert from "node:assert/strict";
import test from "node:test";

import {
  createNamedThreadComposerState,
  reduceNamedThreadComposerState,
} from "./namedThreadComposerState.ts";

function namedState(scopeKey = "general") {
  let state = createNamedThreadComposerState(scopeKey);
  state = reduceNamedThreadComposerState(state, {
    type: "toggle",
    scopeKey,
  });
  return reduceNamedThreadComposerState(state, {
    type: "set-title",
    title: "Plan privado",
  });
}

test("changing channels discards a title before the new scope can send", () => {
  const state = reduceNamedThreadComposerState(namedState(), {
    type: "sync-scope",
    enabled: true,
    scopeKey: "random",
  });

  assert.deepEqual(state, createNamedThreadComposerState("random"));
});

test("entering reply or edit mode discards the pending root title", () => {
  const disabled = reduceNamedThreadComposerState(namedState(), {
    type: "sync-scope",
    enabled: false,
    scopeKey: "general",
  });
  const restored = reduceNamedThreadComposerState(disabled, {
    type: "sync-scope",
    enabled: true,
    scopeKey: "general",
  });

  assert.deepEqual(restored, createNamedThreadComposerState("general"));
});

test("only an explicit successful-send reset clears a same-scope title", () => {
  const pending = namedState();
  const unchanged = reduceNamedThreadComposerState(pending, {
    type: "sync-scope",
    enabled: true,
    scopeKey: "general",
  });
  const sent = reduceNamedThreadComposerState(unchanged, { type: "reset" });

  assert.deepEqual(unchanged, pending);
  assert.deepEqual(sent, createNamedThreadComposerState("general"));
});

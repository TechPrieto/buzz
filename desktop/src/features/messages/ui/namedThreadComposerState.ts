export type NamedThreadComposerState = {
  isNaming: boolean;
  scopeKey: string | null;
  title: string;
};

type NamedThreadComposerAction =
  | { type: "reset" }
  | { type: "set-title"; title: string }
  | { type: "sync-scope"; enabled: boolean; scopeKey: string | null }
  | { type: "toggle"; scopeKey: string | null };

export function createNamedThreadComposerState(
  scopeKey: string | null,
): NamedThreadComposerState {
  return { isNaming: false, scopeKey, title: "" };
}

export function reduceNamedThreadComposerState(
  state: NamedThreadComposerState,
  action: NamedThreadComposerAction,
): NamedThreadComposerState {
  switch (action.type) {
    case "reset":
      return createNamedThreadComposerState(state.scopeKey);
    case "set-title":
      return { ...state, title: action.title };
    case "sync-scope":
      return !action.enabled || state.scopeKey !== action.scopeKey
        ? createNamedThreadComposerState(action.scopeKey)
        : state;
    case "toggle":
      return state.isNaming
        ? createNamedThreadComposerState(action.scopeKey)
        : { isNaming: true, scopeKey: action.scopeKey, title: "" };
  }
}

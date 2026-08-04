import * as React from "react";

import { buildNamedThreadTags } from "@/features/messages/lib/threadConversation";
import { cn } from "@/shared/lib/cn";
import {
  createNamedThreadComposerState,
  reduceNamedThreadComposerState,
} from "./namedThreadComposerState";

type NamedThreadComposerController = {
  additionalTags: string[][] | undefined;
  reset: () => void;
  titleField: React.ReactNode;
  toolbarAction: React.ReactNode;
};

export function useNamedThreadComposer(
  enabled: boolean,
  scopeKey: string | null,
): NamedThreadComposerController {
  const [state, dispatch] = React.useReducer(
    reduceNamedThreadComposerState,
    scopeKey,
    createNamedThreadComposerState,
  );

  React.useEffect(() => {
    dispatch({ type: "sync-scope", enabled, scopeKey });
  }, [enabled, scopeKey]);

  const reset = React.useCallback(() => {
    dispatch({ type: "reset" });
  }, []);

  const isNaming =
    enabled && state.scopeKey === scopeKey && state.isNaming;
  const additionalTags =
    isNaming ? buildNamedThreadTags(state.title) : undefined;
  const titleField =
    enabled && isNaming ? (
      <div className="mb-2 flex items-center gap-2 rounded-lg border border-border/50 bg-muted/30 px-3 py-2">
        <label
          className="shrink-0 text-xs font-medium text-muted-foreground"
          htmlFor="named-thread-title"
        >
          Thread name
        </label>
        <input
          className="min-w-0 flex-1 bg-transparent text-sm outline-none placeholder:text-muted-foreground/60"
          data-testid="named-thread-title"
          id="named-thread-title"
          maxLength={80}
          onChange={(event) =>
            dispatch({ type: "set-title", title: event.target.value })
          }
          placeholder="e.g. Plan agosto — contenido"
          type="text"
          value={state.title}
        />
      </div>
    ) : null;
  const toolbarAction = enabled ? (
    <button
      aria-pressed={isNaming}
      className={cn(
        "rounded-md px-2 py-1 text-xs font-medium transition-colors",
        isNaming
          ? "bg-accent text-accent-foreground"
          : "text-muted-foreground hover:bg-accent hover:text-accent-foreground",
      )}
      onClick={() => {
        dispatch({ type: "toggle", scopeKey });
      }}
      type="button"
    >
      Name thread
    </button>
  ) : null;

  return { additionalTags, reset, titleField, toolbarAction };
}

const MAX_THREAD_TITLE_LENGTH = 80;

function cleanThreadTitle(value: string): string {
  const normalized = value
    .trim()
    .replace(/^#{1,6}\s+/, "")
    .replace(/\s+/g, " ");
  if (normalized.length <= MAX_THREAD_TITLE_LENGTH) {
    return normalized;
  }
  return `${normalized.slice(0, MAX_THREAD_TITLE_LENGTH - 1).trimEnd()}…`;
}

export function buildNamedThreadTags(title: string): string[][] {
  const normalized = cleanThreadTitle(title);
  return normalized ? [["subject", normalized]] : [];
}

/**
 * Resolve the human-facing title of a NIP-10 conversation.
 *
 * The immutable root event remains the canonical identity. An explicit
 * `subject` tag is preferred when a client provides one; existing threads stay
 * compatible by using the first non-empty line of the root message.
 */
export function resolveThreadTitle(
  content: string,
  tags: readonly (readonly string[])[] = [],
): string | null {
  const explicitSubject = tags.find(
    (tag) => tag[0] === "subject" && tag[1]?.trim(),
  )?.[1];
  if (explicitSubject) {
    return cleanThreadTitle(explicitSubject) || null;
  }

  const firstContentLine = content
    .split("\n")
    .map((line) => line.trim())
    .find(Boolean);
  if (!firstContentLine) {
    return null;
  }
  return cleanThreadTitle(firstContentLine) || null;
}

type ThreadAudienceMessage = {
  pubkey?: string;
  signerPubkey?: string;
  tags?: readonly (readonly string[])[];
};

/** Collect agent recipients explicitly addressed by the owner anywhere in a thread. */
export function collectThreadAgentAudience(
  messages: readonly ThreadAudienceMessage[],
  ownerPubkey: string,
  isAgentPubkey: (pubkey: string) => boolean,
): string[] {
  const normalizedOwner = ownerPubkey.trim().toLowerCase();
  const audience: string[] = [];
  const seen = new Set<string>();

  for (const message of messages) {
    const signer = (message.signerPubkey ?? message.pubkey ?? "")
      .trim()
      .toLowerCase();
    if (signer !== normalizedOwner) continue;

    for (const tag of message.tags ?? []) {
      if (tag[0] !== "p") continue;
      const pubkey = tag[1]?.trim().toLowerCase() ?? "";
      if (!pubkey || seen.has(pubkey) || !isAgentPubkey(pubkey)) continue;
      seen.add(pubkey);
      audience.push(pubkey);
    }
  }

  return audience;
}

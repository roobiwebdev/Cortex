import type { StreamDest } from "./types";

/**
 * SOURCE OF TRUTH KEYWORDS: isWhip, WHIP, custom destination, stream URL, token optional, needsKey
 * WHAT:  Whether a destination is a WHIP server (a custom http(s):// URL), and whether it
 *        needs a stored key before Go live can start.
 * WHY:   WHIP servers may take no token, while RTMP(S) always needs a stream key. The engine
 *        applies the same rule (`resolve_destination` in rec-engine-macos/src/live/mod.rs),
 *        so the popover and settings must not block a WHIP server that has no token.
 * WHERE: settings/live.tsx (labels), main-window/go-live.tsx (Go live blocker).
 */
export const isWhip = (d: StreamDest) => d.service === "custom" && /^https?:\/\//i.test(d.url?.trim() ?? "");

export const needsKey = (d: StreamDest) => !d.keyStored && !isWhip(d);

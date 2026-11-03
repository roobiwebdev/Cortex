/**
 * SOURCE OF TRUTH KEYWORDS: playBlip, interface sounds, Web Audio, synthesized, tour step sound, sounds setting
 * WHAT:  A soft synthesized blip (sine, 12 ms attack, exponential decay) for interface moments
 *        such as a tour step. No audio files ship.
 * WHY:   A quiet cue makes stepping through the tour feel physical. Callers check the `sounds`
 *        setting and never play while capturing (system audio would record it).
 * WHERE: the onboarding tour. Fails silently without an audio device.
 */

let ctx: AudioContext | null = null;

function audio(): AudioContext | null {
  if (typeof AudioContext === "undefined") return null;
  try {
    ctx ??= new AudioContext();
    if (ctx.state === "suspended") void ctx.resume();
    return ctx;
  } catch {
    return null;
  }
}

/** `pitch` 0..1 raises the note a little (later tour steps sound slightly brighter). */
export function playBlip(pitch = 0, gain = 0.07) {
  const a = audio();
  if (!a) return;
  try {
    const t = a.currentTime + 0.005;
    const osc = a.createOscillator();
    const amp = a.createGain();
    const freq = 740 + Math.max(0, Math.min(1, pitch)) * 220;
    osc.type = "sine";
    osc.frequency.setValueAtTime(freq, t);
    // A slight upward glide reads as "forward".
    osc.frequency.exponentialRampToValueAtTime(freq * 1.12, t + 0.09);
    amp.gain.setValueAtTime(0.0001, t);
    amp.gain.exponentialRampToValueAtTime(gain, t + 0.012);
    amp.gain.exponentialRampToValueAtTime(0.0001, t + 0.22);
    osc.connect(amp).connect(a.destination);
    osc.start(t);
    osc.stop(t + 0.25);
  } catch {
    // No output device or a closed context: interface sounds are optional.
  }
}

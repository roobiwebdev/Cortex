// Checks the TypeScript mirror against the cases rec-core's Rust test uses, so the pickers
// show the size the engine records. Run with `pnpm --filter desktop test` (plain Node, no deps).
import fixture from "../../../../crates/rec-core/tests/fixtures/output-sizes.json" with { type: "json" };

import { autoBitrate, outputSize, resolveCodec, RESOLUTIONS, type PixelSize } from "./output-size.ts";
import type { Codec } from "./types";

const CODECS: readonly Codec[] = ["auto", "h264", "hevc", "prores"];

function pick<T extends string>(all: readonly T[], v: string): T {
  const hit = all.find((x) => x === v);
  if (hit === undefined) throw new Error(`unknown value ${v}`);
  return hit;
}

function size(pair: readonly number[]): PixelSize {
  const [w, h] = pair;
  if (w === undefined || h === undefined || pair.length !== 2) throw new Error(`bad size ${String(pair)}`);
  return { w, h };
}

const failures: string[] = [];
const expect = (what: string, got: unknown, want: unknown) => {
  if (JSON.stringify(got) !== JSON.stringify(want))
    failures.push(`${what}: got ${JSON.stringify(got)}, want ${JSON.stringify(want)}`);
};

for (const c of fixture.sizes) {
  expect(
    `outputSize ${c.resolution} ${String(c.source)} ${("codec" in c && c.codec) || "auto"}`,
    outputSize(pick(RESOLUTIONS, c.resolution), size(c.source), pick(CODECS, ("codec" in c && c.codec) || "auto")),
    size(c.output),
  );
}
for (const c of fixture.bitrates) {
  expect(
    `autoBitrate ${c.codec} ${String(c.size)} ${c.fps}`,
    autoBitrate(pick(CODECS, c.codec), size(c.size), c.fps),
    c.bps,
  );
}
for (const c of fixture.codecs) {
  expect(`resolveCodec ${c.codec} ${String(c.size)}`, resolveCodec(pick(CODECS, c.codec), size(c.size)), c.resolved);
}

const total = fixture.sizes.length + fixture.bitrates.length + fixture.codecs.length;
if (failures.length > 0) throw new Error(`${failures.length} of ${total} cases failed:\n${failures.join("\n")}`);
console.log(`output-size: ${total} shared cases pass`);

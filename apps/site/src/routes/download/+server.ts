import { DOWNLOAD_URL, DOWNLOADS } from "#lib/site.ts";

import type { RequestHandler } from "./$types";

export const prerender = false;

/** The asset for `?os=windows`, `?os=linux&arch=aarch64&format=deb`…; the Mac DMG otherwise. */
function pick(params: URLSearchParams): string {
  const os = params.get("os");
  if (os === "windows")
    return params.get("format") === "msi" ? DOWNLOADS.windows.x86_64.msi.url : DOWNLOADS.windows.x86_64.exe.url;
  if (os === "linux") {
    const builds = params.get("arch") === "aarch64" ? DOWNLOADS.linux.aarch64 : DOWNLOADS.linux.x86_64;
    const format = params.get("format");
    if (format === "deb") return builds.deb.url;
    if (format === "rpm") return builds.rpm.url;
    return builds.appimage.url;
  }
  return DOWNLOAD_URL;
}

export const GET: RequestHandler = ({ url }) =>
  new Response(null, {
    status: 302,
    headers: { location: pick(url.searchParams), "cache-control": "no-store" },
  });

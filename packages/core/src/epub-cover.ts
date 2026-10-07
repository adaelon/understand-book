import { strFromU8 } from "fflate";
import { parse } from "node-html-parser";
import { posix as path } from "node:path";

const imageTypes: Record<string, string> = {
  ".jpg": "image/jpeg", ".jpeg": "image/jpeg", ".png": "image/png",
  ".webp": "image/webp", ".gif": "image/gif", ".svg": "image/svg+xml", ".avif": "image/avif",
};

// Cover declarations may point outside the spine; never pick a body illustration.
export function extractEpubCover(files: Record<string, Uint8Array>): { bytes: Uint8Array; extension: string; mime: string } | null {
  const document = (name: string) => files[name] ? parse(strFromU8(files[name])) : null;
  const resolve = (base: string, href: string) => {
    if (/^[a-z][a-z0-9+.-]*:/i.test(href)) return "";
    try { return path.normalize(path.join(path.dirname(base), decodeURIComponent(href.split(/[?#]/)[0]))); }
    catch { return ""; }
  };
  const opfPath = document("META-INF/container.xml")?.querySelector("rootfile")?.getAttribute("full-path");
  if (!opfPath) return null;
  const opf = document(opfPath);
  if (!opf) return null;
  const items = opf.querySelectorAll("manifest item");
  const coverId = opf.querySelectorAll("metadata meta").find(item => item.getAttribute("name") === "cover")?.getAttribute("content");
  const declared = items.find(item => item.getAttribute("properties")?.split(/\s+/).includes("cover-image"))
    ?? items.find(item => coverId && item.getAttribute("id") === coverId);
  const href = declared?.getAttribute("href")
    ?? opf.querySelectorAll("guide reference").find(item => item.getAttribute("type")?.split(/\s+/).includes("cover"))?.getAttribute("href");
  if (!href) return null;
  let file = resolve(opfPath, href);
  if (!imageTypes[path.extname(file).toLowerCase()]) {
    const page = document(file);
    const image = page?.querySelector("img")?.getAttribute("src")
      ?? page?.querySelector("image")?.getAttribute("xlink:href")
      ?? page?.querySelector("image")?.getAttribute("href");
    if (!image) return null;
    file = resolve(file, image);
  }
  const extension = path.extname(file).toLowerCase(), mime = imageTypes[extension];
  return mime && files[file]?.length ? { bytes: files[file], extension, mime } : null;
}

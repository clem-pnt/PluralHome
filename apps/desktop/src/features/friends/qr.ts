import jsQR from "jsqr";
import { toDataURL } from "qrcode";

/** QR code en image `data:` (la CSP n'autorise ni blob: ni canvas externe). */
export function qrDataUrl(text: string): Promise<string> {
  return toDataURL(text, { errorCorrectionLevel: "M", margin: 2, width: 280 });
}

const MAX_SIDE = 1600;

/** Lit le premier QR code d'un canevas, ou `null`. */
export function readCanvas(canvas: HTMLCanvasElement): string | null {
  const ctx = canvas.getContext("2d", { willReadFrequently: true });
  if (!ctx || canvas.width === 0 || canvas.height === 0) return null;
  const img = ctx.getImageData(0, 0, canvas.width, canvas.height);
  return jsQR(img.data, img.width, img.height)?.data ?? null;
}

/** Lit un QR code dans une image choisie (capture d'écran, photo). */
export async function readImageFile(file: File): Promise<string | null> {
  const url = await new Promise<string>((resolve, reject) => {
    const r = new FileReader();
    r.onload = () => resolve(String(r.result));
    r.onerror = () => reject(r.error);
    r.readAsDataURL(file);
  });
  const img = new Image();
  img.src = url;
  await img.decode();
  const scale = Math.min(1, MAX_SIDE / Math.max(img.naturalWidth, img.naturalHeight));
  const canvas = document.createElement("canvas");
  canvas.width = Math.round(img.naturalWidth * scale);
  canvas.height = Math.round(img.naturalHeight * scale);
  canvas.getContext("2d")?.drawImage(img, 0, 0, canvas.width, canvas.height);
  return readCanvas(canvas);
}

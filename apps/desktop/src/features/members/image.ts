/** Taille et poids visés : de quoi rester net en 96 px sur un écran haute densité. */
const SIDE = 256;
const QUALITY = 0.86;

/**
 * Recadre l'image au carré (centre), la réduit à 256 px et la réencode : les
 * métadonnées (position GPS, appareil…) disparaissent au passage.
 * Renvoie le contenu en base64 (pour le cœur) et une adresse data: (pour l'aperçu).
 */
export async function squareAvatar(file: Blob): Promise<{ data: string; preview: string }> {
  const bitmap = await createImageBitmap(file);
  const side = Math.min(bitmap.width, bitmap.height);
  const canvas = document.createElement("canvas");
  canvas.width = SIDE;
  canvas.height = SIDE;
  const ctx = canvas.getContext("2d");
  if (!ctx) throw new Error("canvas");
  ctx.imageSmoothingQuality = "high";
  ctx.drawImage(
    bitmap,
    (bitmap.width - side) / 2,
    (bitmap.height - side) / 2,
    side,
    side,
    0,
    0,
    SIDE,
    SIDE,
  );
  bitmap.close();
  const encode = (type: string) =>
    new Promise<Blob | null>((resolve) => canvas.toBlob(resolve, type, QUALITY));
  // WebP si le moteur sait l'écrire, sinon JPEG (WebKitGTK selon les versions).
  let blob = await encode("image/webp");
  if (blob?.type !== "image/webp") blob = await encode("image/jpeg");
  if (!blob) throw new Error("encode");
  const bytes = new Uint8Array(await blob.arrayBuffer());
  let binary = "";
  for (const b of bytes) binary += String.fromCharCode(b);
  const data = btoa(binary);
  return { data, preview: `data:${blob.type};base64,${data}` };
}

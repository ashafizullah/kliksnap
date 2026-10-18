/** Corner rounding of the webcam's shapes in Settings. */
export const RADIUS: Record<string, string> = { circle: "50%", rounded: "18%", square: "0" };

const SQUARE = { width: { ideal: 640 }, height: { ideal: 640 } };

export type Camera = { id: string; label: string };

/** The cameras, by name. Names are only given once the camera is allowed, so this asks for it (briefly). */
export async function listCameras(): Promise<Camera[]> {
  const cameras = async () =>
    (await navigator.mediaDevices.enumerateDevices())
      .filter((d) => d.kind === "videoinput" && d.deviceId)
      .map((d) => ({ id: d.deviceId, label: d.label }));
  let found = await cameras();
  if (found.length && found.every((c) => c.label)) return found;
  const s = await navigator.mediaDevices.getUserMedia({ video: true, audio: false });
  s.getTracks().forEach((t) => t.stop());
  found = await cameras();
  return found;
}

/**
 * The camera with this ID, or else this name (IDs can change), or else the
 * system's default one when `id` is empty or the camera is gone.
 */
export async function openCamera(id: string, label: string): Promise<MediaStream> {
  const open = (deviceId?: string) =>
    navigator.mediaDevices.getUserMedia({
      video: deviceId ? { ...SQUARE, deviceId: { exact: deviceId } } : { ...SQUARE, facingMode: "user" },
      audio: false,
    });
  if (!id) return open();
  try {
    return await open(id);
  } catch (e) {
    if ((e as DOMException)?.name === "NotAllowedError") throw e;
  }
  const fallback = await open();
  const same = label && (await listCameras()).find((c) => c.label === label);
  if (!same || fallback.getVideoTracks()[0]?.getSettings().deviceId === same.id) return fallback;
  fallback.getTracks().forEach((t) => t.stop());
  return open(same.id);
}

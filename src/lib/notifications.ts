const isTauri = () => "__TAURI_INTERNALS__" in window;

export async function enableNativeNotifications(): Promise<boolean> {
  if (!isTauri()) return true;
  const { isPermissionGranted, requestPermission } = await import("@tauri-apps/plugin-notification");
  if (await isPermissionGranted()) return true;
  return await requestPermission() === "granted";
}

export async function sendNativeNotification(title: string, body: string): Promise<void> {
  if (!isTauri()) return;
  const { isPermissionGranted, sendNotification } = await import("@tauri-apps/plugin-notification");
  if (await isPermissionGranted()) sendNotification({ title, body });
}

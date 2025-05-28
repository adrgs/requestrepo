import { toast } from "react-toastify";

const toastCache = new Map<string, number>();
const TOAST_THROTTLE_MS = 5000; // 5 seconds

export function throttledToastError(message: string, options = {}): void {
  const now = Date.now();
  const lastShown = toastCache.get(message) || 0;
  
  if (now - lastShown > TOAST_THROTTLE_MS) {
    toastCache.set(message, now);
    toast.error(message, options);
  }
}

export function throttledToastSuccess(message: string, options = {}): void {
  const now = Date.now();
  const lastShown = toastCache.get(message) || 0;
  
  if (now - lastShown > TOAST_THROTTLE_MS) {
    toastCache.set(message, now);
    toast.success(message, options);
  }
}

export function throttledToastInfo(message: string, options = {}): void {
  const now = Date.now();
  const lastShown = toastCache.get(message) || 0;
  
  if (now - lastShown > TOAST_THROTTLE_MS) {
    toastCache.set(message, now);
    toast.info(message, options);
  }
}

export function throttledToastWarning(message: string, options = {}): void {
  const now = Date.now();
  const lastShown = toastCache.get(message) || 0;
  
  if (now - lastShown > TOAST_THROTTLE_MS) {
    toastCache.set(message, now);
    toast.warning(message, options);
  }
}

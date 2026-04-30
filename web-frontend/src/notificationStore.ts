import { createRoot, createSignal } from "solid-js";

function createNotificationStore() {
  const [notification, setNotification] = createSignal("");

  function showNotification(msg: string, duration = 2500) {
    setNotification(msg);
    if (duration > 0) {
      setTimeout(() => setNotification(""), duration);
    }
  }

  function clearNotification() {
    setNotification("");
  }

  return { notification, showNotification, clearNotification };
}

export const notificationStore = createRoot(createNotificationStore);

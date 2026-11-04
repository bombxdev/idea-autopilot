// The app's own confirmation box (in place of the browser's confirm()). `await ask("...")` → true / false.
// Options: title, ok (button text), cancel, danger (red button), code (a command shown in a box).
export const dialog = $state({ open: null });

export function ask(message, opts = {}) {
  return new Promise((resolve) => {
    dialog.open = { message, title: "", ok: "ตกลง", cancel: "ยกเลิก", danger: false, code: "", ...opts, resolve };
  });
}

export function answer(v) {
  const d = dialog.open;
  dialog.open = null;
  d?.resolve(v);
}

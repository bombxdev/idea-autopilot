// A textarea that grows with what is typed (like Claude's prompt box) up to a share of the window, then scrolls
// (ISSUE-078). `use:autogrow={[0.45, text]}`: 0.45 = at most 45% of the window height (0 = no limit); passing the
// text makes it re-fit when the text is changed from code too.
export function autogrow(node, arg) {
  let frac = Array.isArray(arg) ? arg[0] : arg;
  const fit = () => {
    const max = frac > 0 ? Math.round(window.innerHeight * frac) : Infinity;
    node.style.height = "auto";
    const want = node.scrollHeight + 2;
    node.style.height = Math.min(want, max) + "px";
    node.style.overflowY = want > max ? "auto" : "hidden";
  };
  node.addEventListener("input", fit);
  window.addEventListener("resize", fit);
  requestAnimationFrame(fit);
  return {
    update(a) {
      frac = Array.isArray(a) ? a[0] : a;
      requestAnimationFrame(fit);
    },
    destroy() {
      node.removeEventListener("input", fit);
      window.removeEventListener("resize", fit);
    },
  };
}

// Code blocks inside a plain textarea, like Claude's prompt box (ISSUE-082, ISSUE-084):
// - the textarea's own text is invisible; a layer on top (pointer-events: none) draws the same text in the same
//   place, so Thai input, pasting images, the caret and selection all stay the textarea's own
// - three backticks on a line open a block (the closing fence is added); the fence lines are not drawn — they
//   become the block's top and bottom padding, and the caret never stops on them
// - Enter inside a block is a new line (never sends); Enter on an empty last line leaves the block
// - Backspace at the start of a block removes the block (an empty one entirely; otherwise its text stays as text)
// - an inline code span (single backticks) is drawn with a light background, its backticks hidden
// `use:codebox={text}` — pass the text so changes made from code repaint too.
// Backticks are built from char 96 (a literal one confuses the i18n extractor).
const TICK = String.fromCharCode(96);
const FENCE = TICK.repeat(3);
const isFence = (line) => line.trimStart().startsWith(FENCE);
const escHtml = (s) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
const INLINE = new RegExp(TICK + "([^" + TICK + "\\n]+)" + TICK, "g");

/** lines with their bounds and fence role: "open" | "close" | "" (an unclosed block runs to the end) */
export function scan(v) {
  const out = [];
  let at = 0, inside = false;
  for (const text of v.split("\n")) {
    let role = "";
    if (isFence(text)) {
      role = inside ? "close" : "open";
      inside = !inside;
    }
    out.push({ start: at, end: at + text.length, text, role, inBlock: inside && role === "" });
    at += text.length + 1;
  }
  return out;
}

const inlineHtml = (line) =>
  escHtml(line).replace(INLINE, (_, x) => `<span class="cb-tick">${TICK}</span><span class="cb-inline">${x}</span><span class="cb-tick">${TICK}</span>`);

/** the overlay's html: the same text, each block in a box, fences and inline backticks invisible */
export function paintBlocks(text) {
  const lines = scan(text);
  let html = "";
  lines.forEach((l, i) => {
    const nl = i < lines.length - 1 ? "\n" : "";
    if (l.role === "open") html += `<div class="cb-block"><span class="cb-fence">${escHtml(l.text)}</span>${nl}`;
    else if (l.role === "close") html += `<span class="cb-fence">${escHtml(l.text)}</span>${nl}</div>`;
    else html += (l.inBlock ? escHtml(l.text) : inlineHtml(l.text)) + nl;
  });
  if (lines.some((l) => l.role === "open") && lines.filter((l) => l.role).length % 2 === 1) html += "</div>";
  // a trailing newline shows as an empty line in a textarea but not in a div
  return html + "​";
}

const lineAt = (lines, pos) => {
  for (let i = 0; i < lines.length; i++) if (pos <= lines[i].end) return i;
  return lines.length - 1;
};

export function codebox(node) {
  const parent = node.parentNode;
  const top = document.createElement("div");
  top.className = "cb-back";
  top.setAttribute("aria-hidden", "true");
  parent.insertBefore(top, node.nextSibling);
  if (getComputedStyle(parent).position === "static") parent.style.position = "relative";
  node.classList.add("cb-on");
  let busy = false, dir = "";

  function place() {
    const cs = getComputedStyle(node);
    for (const k of ["fontFamily", "fontSize", "fontWeight", "lineHeight", "letterSpacing", "wordSpacing", "tabSize",
      "paddingTop", "paddingRight", "paddingBottom", "paddingLeft", "textIndent", "textTransform", "direction"]) top.style[k] = cs[k];
    top.style.left = node.offsetLeft + parseFloat(cs.borderLeftWidth) + "px";
    top.style.top = node.offsetTop + parseFloat(cs.borderTopWidth) + "px";
    top.style.width = node.clientWidth + "px";
    top.style.height = node.clientHeight + "px";
  }
  function paint() {
    place();
    top.innerHTML = paintBlocks(node.value);
    top.scrollTop = node.scrollTop;
  }
  // write through the textarea itself so undo and the page's own bindings (bind:value, autogrow) follow
  function put(text, from, to, caret) {
    busy = true;
    node.setRangeText(text, from, to, "end");
    node.setSelectionRange(caret, caret);
    node.dispatchEvent(new Event("input", { bubbles: true }));
    busy = false;
    paint();
  }
  const caretTo = (p) => node.setSelectionRange(p, p);

  // the caret never rests on a hidden fence line: step over it in the direction it was moving
  function skipFence() {
    if (document.activeElement !== node || node.selectionStart !== node.selectionEnd) return;
    const v = node.value, lines = scan(v), i = lineAt(lines, node.selectionStart), l = lines[i];
    if (!l.role) return;
    const up = dir === "up";
    if (l.role === "open") {
      if (up && i > 0) caretTo(lines[i - 1].end);
      else if (lines[i + 1]) caretTo(lines[i + 1].start);
    } else if (!up) {
      if (lines[i + 1]) caretTo(lines[i + 1].start);
      else put("\n", v.length, v.length, v.length + 1); // nothing below the block yet: make a line to type on
    } else caretTo(lines[i - 1].end);
  }

  function onInput(e) {
    if (busy) return;
    // any insert (one key, several chars at once, or a phone keyboard's composed text), never a delete
    if (!e.isComposing && (!e.inputType || e.inputType.startsWith("insert")) && (e.data == null || e.data.includes(TICK))) {
      const v = node.value, pos = node.selectionStart, lines = scan(v), l = lines[lineAt(lines, pos)];
      // the line is exactly the three backticks just typed, and that fence opens a block
      if (l.text === FENCE && l.end === pos && l.role === "open" && !lines.some((x) => x.role === "close" && x.start > l.start)) {
        put("\n\n" + FENCE, pos, pos, pos + 1);
        return;
      }
    }
    paint();
  }
  function onKey(e) {
    dir = ["ArrowUp", "ArrowLeft", "PageUp", "Home"].includes(e.key) ? "up" : ["ArrowDown", "ArrowRight", "PageDown", "End"].includes(e.key) ? "down" : "";
    if (e.isComposing || e.ctrlKey || e.metaKey || e.altKey) return;
    const v = node.value, pos = node.selectionStart;
    if (pos !== node.selectionEnd) return;
    const lines = scan(v), i = lineAt(lines, pos), l = lines[i], prev = lines[i - 1], next = lines[i + 1];
    if (e.key === "Enter" && !e.shiftKey && l.inBlock) {
      e.preventDefault();
      e.stopPropagation();
      if (l.text.trim() === "" && next?.role === "close" && prev?.role !== "open") {
        // empty last line of the block: drop it and continue below the block
        const rest = v.slice(next.end);
        const out = v.slice(0, l.start) + next.text + (rest.startsWith("\n") ? rest : "\n" + rest);
        put(out, 0, v.length, l.start + next.text.length + 1);
      } else put("\n", pos, pos, pos + 1);
      return;
    }
    if (e.key === "Backspace" && pos === l.start && prev?.role === "open") {
      // at the start of a block: remove it — an empty block entirely, otherwise just its fences
      e.preventDefault();
      e.stopPropagation();
      let c = i;
      while (lines[c] && lines[c].role !== "close") c++;
      const body = lines.slice(i, c);
      const from = prev.start, to = lines[c] ? lines[c].end : v.length;
      const keep = body.every((x) => !x.text.trim()) ? "" : body.map((x) => x.text).join("\n");
      // drop the line break the block leaves behind too
      const cut = keep ? [from, to] : from > 0 ? [from - 1, to] : [0, Math.min(to + 1, v.length)];
      put(keep, cut[0], cut[1], cut[0]);
      return;
    }
    if (e.key === "Backspace" && pos === l.start && prev?.role === "close") {
      // just below a block: step into its last line instead of breaking the fence
      e.preventDefault();
      caretTo(lines[i - 2] ? lines[i - 2].end : prev.start);
      return;
    }
    if (e.key === "Delete" && pos === l.end && (next?.role === "open" || next?.role === "close")) {
      e.preventDefault(); // would pull a fence into this line
    }
  }
  const onMouse = () => (dir = "");
  const onSel = () => requestAnimationFrame(skipFence);
  const onScroll = () => (top.scrollTop = node.scrollTop);
  node.addEventListener("input", onInput);
  node.addEventListener("keydown", onKey);
  node.addEventListener("mousedown", onMouse);
  node.addEventListener("scroll", onScroll);
  document.addEventListener("selectionchange", onSel);
  const ro = new ResizeObserver(paint);
  ro.observe(node);
  requestAnimationFrame(paint);
  return {
    update() {
      requestAnimationFrame(paint);
    },
    destroy() {
      node.removeEventListener("input", onInput);
      node.removeEventListener("keydown", onKey);
      node.removeEventListener("mousedown", onMouse);
      node.removeEventListener("scroll", onScroll);
      document.removeEventListener("selectionchange", onSel);
      ro.disconnect();
      top.remove();
      node.classList.remove("cb-on");
    },
  };
}

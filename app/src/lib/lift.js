// Full-screen panels: while `on`, the node is moved to <body>, so `position: fixed` is measured against the window and
// not against a drawer that is animated or scrolls (inside one it overflowed the screen and lost its scrollbar).
// A comment marks its place so it goes back exactly where it was.
export function lift(node, on) {
  const marker = document.createComment("lift");
  node.before(marker);
  const apply = (v) => (v ? document.body.appendChild(node) : marker.after(node));
  apply(on);
  return { update: apply, destroy() { marker.remove(); if (node.parentNode === document.body) node.remove(); } };
}

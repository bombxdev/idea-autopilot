<script>
  // The dashboard's 3D studio (ISSUE-173): a small cartoon office in three.js. Every idea in work gets a desk; at it
  // sits an AI character — one per AI (body in the vendor's colour, a head with the vendor's glyph), or the team's
  // characters with their own faces. They move by what is happening: typing while working, a thought bubble while
  // thinking, a raised hand when waiting for the owner, a red lamp on trouble. A label shows the idea and its %.
  // Light on the machine: low-poly, ~30 fps, nothing drawn while the window is hidden; click a desk to open the idea.
  import { onMount } from "svelte";
  import * as THREE from "three";
  import { vendorOf, glyphSvg } from "../lib/vendors.js";
  import { avatarSvg } from "../lib/avatar.js";

  let { items = [], onopen = null } = $props();
  let box;
  let tip = $state(null);

  const dark = () => {
    const bg = getComputedStyle(document.body).backgroundColor.match(/\d+/g)?.map(Number) ?? [20, 20, 20];
    return bg[0] + bg[1] + bg[2] < 384;
  };

  function svgTexture(svg, px = 128) {
    const img = new Image();
    const tex = new THREE.Texture();
    tex.colorSpace = THREE.SRGBColorSpace;
    img.onload = () => {
      const c = document.createElement("canvas");
      c.width = c.height = px;
      c.getContext("2d").drawImage(img, 0, 0, px, px);
      tex.image = c;
      tex.needsUpdate = true;
    };
    img.src = "data:image/svg+xml;charset=utf-8," + encodeURIComponent(svg);
    return tex;
  }

  function labelTexture(title, pctText, color) {
    const c = document.createElement("canvas");
    c.width = 512;
    c.height = 112;
    const g = c.getContext("2d");
    g.fillStyle = dark() ? "rgba(22,26,32,0.92)" : "rgba(255,255,255,0.95)";
    g.beginPath();
    g.roundRect(4, 4, 504, 104, 26);
    g.fill();
    g.fillStyle = color;
    g.fillRect(24, 84, 464 * Math.max(0.02, Math.min(1, parseInt(pctText) / 100 || 0)), 8);
    g.fillStyle = dark() ? "#e8edf2" : "#1b2430";
    g.font = "600 34px 'Noto Sans Thai', 'Segoe UI', sans-serif";
    const t = title.length > 22 ? title.slice(0, 21) + "…" : title;
    g.fillText(t, 24, 52);
    g.fillStyle = color;
    g.textAlign = "right";
    g.fillText(pctText, 488, 52);
    const tex = new THREE.CanvasTexture(c);
    tex.colorSpace = THREE.SRGBColorSpace;
    return tex;
  }

  function bubbleTexture(text, color) {
    const c = document.createElement("canvas");
    c.width = c.height = 128;
    const g = c.getContext("2d");
    g.fillStyle = "#ffffff";
    g.beginPath();
    g.arc(64, 56, 48, 0, Math.PI * 2);
    g.fill();
    g.beginPath();
    g.moveTo(40, 92);
    g.lineTo(30, 120);
    g.lineTo(62, 100);
    g.fill();
    g.fillStyle = color;
    g.font = "bold 56px 'Segoe UI', sans-serif";
    g.textAlign = "center";
    g.textBaseline = "middle";
    g.fillText(text, 64, 58);
    const tex = new THREE.CanvasTexture(c);
    tex.colorSpace = THREE.SRGBColorSpace;
    return tex;
  }

  const STATUS_COLOR = { working: "#5dcaa5", thinking: "#6aa8ec", waiting: "#f2b04a", error: "#e2574c", idle: "#8a94a3" };

  onMount(() => {
    const renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true });
    renderer.setPixelRatio(Math.min(2, window.devicePixelRatio || 1));
    renderer.shadowMap.enabled = true;
    renderer.shadowMap.type = THREE.PCFSoftShadowMap;
    box.appendChild(renderer.domElement);

    const scene = new THREE.Scene();
    const camera = new THREE.OrthographicCamera(-10, 10, 10, -10, 0.1, 200);
    camera.position.set(14, 14, 14);
    camera.lookAt(0, 0, 0);

    scene.add(new THREE.HemisphereLight(0xffffff, dark() ? 0x334455 : 0xbfd4e6, dark() ? 1.4 : 1.8));
    const sun = new THREE.DirectionalLight(0xffffff, dark() ? 1.6 : 2.2);
    sun.position.set(8, 16, 6);
    sun.castShadow = true;
    sun.shadow.mapSize.set(1024, 1024);
    Object.assign(sun.shadow.camera, { left: -16, right: 16, top: 16, bottom: -16 });
    scene.add(sun);

    const floor = new THREE.Mesh(new THREE.CircleGeometry(40, 48), new THREE.MeshStandardMaterial({ color: dark() ? 0x1e2a30 : 0xe7eef3, roughness: 1 }));
    floor.rotation.x = -Math.PI / 2;
    floor.receiveShadow = true;
    scene.add(floor);

    const mat = (c, o = {}) => new THREE.MeshStandardMaterial({ color: c, roughness: 0.7, ...o });
    const mesh = (geo, m, shadow = true) => { const x = new THREE.Mesh(geo, m); x.castShadow = shadow; x.receiveShadow = true; return x; };

    let world = new THREE.Group();
    scene.add(world);
    let actors = []; // { kind, parts…, status, phase }
    let pickables = [];
    let sig = "";

    function character(colorHex, faceTex, x, z, facing) {
      const g = new THREE.Group();
      const body = mesh(new THREE.CapsuleGeometry(0.28, 0.45, 6, 12), mat(colorHex));
      body.position.y = 0.75;
      g.add(body);
      const head = new THREE.Sprite(new THREE.SpriteMaterial({ map: faceTex, transparent: true }));
      head.scale.set(0.82, 0.82, 1);
      head.position.y = 1.42;
      g.add(head);
      const arm = (side) => {
        const pivot = new THREE.Group();
        pivot.position.set(0.3 * side, 1.0, 0);
        const a = mesh(new THREE.CapsuleGeometry(0.08, 0.32, 4, 8), mat(colorHex));
        a.position.y = -0.22;
        pivot.add(a);
        g.add(pivot);
        return pivot;
      };
      const armL = arm(-1), armR = arm(1);
      const chair = mesh(new THREE.BoxGeometry(0.6, 0.1, 0.6), mat(0x3b4452));
      chair.position.y = 0.42;
      g.add(chair);
      g.position.set(x, 0, z);
      g.rotation.y = facing;
      return { g, body, head, armL, armR };
    }

    function build(list) {
      world.traverse((o) => { o.geometry?.dispose?.(); if (o.material) { o.material.map?.dispose?.(); o.material.dispose?.(); } });
      scene.remove(world);
      world = new THREE.Group();
      scene.add(world);
      actors = [];
      pickables = [];
      const n = Math.max(1, list.length);
      const cols = Math.ceil(Math.sqrt(n));
      const gap = 4.4;
      list.forEach((it, i) => {
        const v = vendorOf(it.vendor);
        const col = i % cols, row = Math.floor(i / cols);
        const cx = (col - (cols - 1) / 2) * gap, cz = (row - (Math.ceil(n / cols) - 1) / 2) * gap;
        const desk = new THREE.Group();
        desk.position.set(cx, 0, cz);
        world.add(desk);
        const top = mesh(new THREE.BoxGeometry(2.4, 0.14, 1.3), mat(dark() ? 0x8b6b4a : 0xc9a27a));
        top.position.y = 0.95;
        desk.add(top);
        for (const [lx, lz] of [[-1.05, -0.5], [1.05, -0.5], [-1.05, 0.5], [1.05, 0.5]]) {
          const leg = mesh(new THREE.BoxGeometry(0.1, 0.9, 0.1), mat(0x5a4632));
          leg.position.set(lx, 0.45, lz);
          desk.add(leg);
        }
        // monitor: its screen is a progress bar in the vendor's colour
        const mon = mesh(new THREE.BoxGeometry(1.1, 0.7, 0.06), mat(0x22262e));
        mon.position.set(0, 1.45, -0.4);
        desk.add(mon);
        const screen = new THREE.Mesh(new THREE.PlaneGeometry(1.0, 0.6), new THREE.MeshBasicMaterial({ color: new THREE.Color(v.color).multiplyScalar(0.35) }));
        screen.position.set(0, 1.45, -0.366);
        desk.add(screen);
        const fill = new THREE.Mesh(new THREE.PlaneGeometry(0.9 * Math.max(0.03, it.pct / 100), 0.08), new THREE.MeshBasicMaterial({ color: v.color }));
        fill.position.set(-0.45 + (0.9 * Math.max(0.03, it.pct / 100)) / 2, 1.3, -0.364);
        desk.add(fill);
        const stand = mesh(new THREE.BoxGeometry(0.1, 0.3, 0.1), mat(0x22262e));
        stand.position.set(0, 1.12, -0.4);
        desk.add(stand);
        // status lamp
        const lampMat = new THREE.MeshStandardMaterial({ color: STATUS_COLOR[it.status] ?? "#8a94a3", emissive: STATUS_COLOR[it.status] ?? "#8a94a3", emissiveIntensity: 0.8 });
        const lamp = mesh(new THREE.SphereGeometry(0.12, 16, 12), lampMat);
        lamp.position.set(0.95, 1.12, -0.35);
        desk.add(lamp);
        // who works here: the team's characters, else one AI with the vendor's glyph
        const crew = it.team?.member_roles?.filter((m) => m.look).slice(0, 3) ?? [];
        const seats = crew.length ? crew.map((m, k) => [(k - (crew.length - 1) / 2) * 0.9, 1.15]) : [[0, 1.15]];
        const faces = crew.length
          ? crew.map((m) => ({ color: m.look.shirt || v.color, tex: svgTexture(avatarSvg(m.look, { size: 128, bg: "#ffffff00" })) }))
          : [{ color: v.color, tex: svgTexture(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" width="128" height="128"><circle cx="10" cy="10" r="9.6" fill="${v.color}"/>${glyphSvg(v.glyph)}</svg>`) }];
        seats.forEach(([sx, sz], k) => {
          const c = character(faces[k].color, faces[k].tex, sx, sz, Math.PI);
          desk.add(c.g);
          actors.push({ ...c, status: it.status, phase: Math.random() * 6, lamp: lampMat });
        });
        // bubble over the desk for thinking / waiting / trouble
        const sym = { thinking: ["…", "#6aa8ec"], waiting: ["!", "#f2b04a"], error: ["!", "#e2574c"] }[it.status];
        if (sym) {
          const b = new THREE.Sprite(new THREE.SpriteMaterial({ map: bubbleTexture(...sym), transparent: true }));
          b.scale.set(0.7, 0.7, 1);
          b.position.set(0.6, 2.35, 1.0);
          desk.add(b);
          actors.push({ bubble: b, status: it.status, phase: Math.random() * 6 });
        }
        const label = new THREE.Sprite(new THREE.SpriteMaterial({ map: labelTexture(`#${it.id} ${it.title}`, `${it.pct}%`, v.color), transparent: true, depthTest: false }));
        label.scale.set(2.6, 0.57, 1);
        label.position.set(0, 2.9, 0);
        label.renderOrder = 10;
        desk.add(label);
        top.userData = { id: it.id, title: it.title, step: it.step, model: it.model };
        pickables.push(top, mon);
        mon.userData = top.userData;
      });
      // fit the camera to the room
      const span = Math.max(4.2, Math.ceil(Math.sqrt(n)) * gap * 0.52);
      Object.assign(camera, { left: -span * aspect, right: span * aspect, top: span, bottom: -span });
      camera.updateProjectionMatrix();
    }

    let aspect = 1;
    function resize() {
      const w = box.clientWidth, h = box.clientHeight;
      if (!w || !h) return;
      aspect = w / h;
      renderer.setSize(w, h);
      const span = camera.top;
      Object.assign(camera, { left: -span * aspect, right: span * aspect });
      camera.updateProjectionMatrix();
    }
    const ro = new ResizeObserver(resize);
    ro.observe(box);
    resize();

    // rebuild when what is shown changes (not on every tick)
    const unwatch = $effect.root(() => {
      $effect(() => {
        const list = items;
        const s = JSON.stringify(list.map((x) => [x.key, x.status, x.pct, x.title, x.team?.member_roles?.map((m) => m.persona) ?? []]));
        if (s !== sig) {
          sig = s;
          build(list);
        }
      });
    });

    // hover and click
    const ray = new THREE.Raycaster();
    const mouse = new THREE.Vector2();
    function hit(e) {
      const r = renderer.domElement.getBoundingClientRect();
      mouse.set(((e.clientX - r.left) / r.width) * 2 - 1, -((e.clientY - r.top) / r.height) * 2 + 1);
      ray.setFromCamera(mouse, camera);
      return ray.intersectObjects(pickables, false)[0]?.object?.userData;
    }
    const move = (e) => {
      const u = hit(e);
      tip = u ? { ...u, x: e.offsetX, y: e.offsetY } : null;
      renderer.domElement.style.cursor = u ? "pointer" : "default";
    };
    const click = (e) => { const u = hit(e); if (u) onopen?.(u.id); };
    renderer.domElement.addEventListener("mousemove", move);
    renderer.domElement.addEventListener("click", click);

    // ~30 fps, nothing while hidden
    let raf = 0, last = 0;
    const clock = new THREE.Clock();
    function frame(now) {
      raf = requestAnimationFrame(frame);
      if (document.hidden || now - last < 33) return;
      last = now;
      const t = clock.getElapsedTime();
      for (const a of actors) {
        const p = t * 1 + a.phase;
        if (a.bubble) { a.bubble.position.y = 2.35 + Math.sin(p * 2) * 0.08; continue; }
        if (a.status === "working") {
          a.armL.rotation.x = -1.1 + Math.sin(p * 14) * 0.18;
          a.armR.rotation.x = -1.1 + Math.cos(p * 14) * 0.18;
          a.body.position.y = 0.75 + Math.abs(Math.sin(p * 7)) * 0.02;
          a.head.position.y = 1.42 + Math.sin(p * 3.5) * 0.02;
        } else if (a.status === "thinking") {
          a.armL.rotation.x = -0.4;
          a.armR.rotation.x = -2.3 + Math.sin(p) * 0.1; // hand to the chin
          a.head.position.x = Math.sin(p * 0.8) * 0.05;
        } else if (a.status === "waiting") {
          a.armR.rotation.x = -3.0;
          a.armR.rotation.z = Math.sin(p * 6) * 0.35; // waving
          a.armL.rotation.x = -0.2;
        } else if (a.status === "error") {
          a.lamp.emissiveIntensity = Math.sin(p * 8) > 0 ? 1.6 : 0.1;
          a.armL.rotation.x = a.armR.rotation.x = -0.6;
          a.head.position.x = Math.sin(p * 10) * 0.03;
        } else {
          a.armL.rotation.x = a.armR.rotation.x = -0.3 + Math.sin(p) * 0.05;
          a.body.rotation.z = Math.sin(p * 0.6) * 0.04;
        }
      }
      world.rotation.y = Math.sin(t * 0.08) * 0.08; // the room drifts a little, to feel alive
      renderer.render(scene, camera);
    }
    raf = requestAnimationFrame(frame);

    return () => {
      cancelAnimationFrame(raf);
      unwatch();
      ro.disconnect();
      renderer.domElement.removeEventListener("mousemove", move);
      renderer.domElement.removeEventListener("click", click);
      scene.traverse((o) => { o.geometry?.dispose?.(); if (o.material) { o.material.map?.dispose?.(); o.material.dispose?.(); } });
      renderer.dispose();
      renderer.domElement.remove();
    };
  });
</script>

<div class="s3d" bind:this={box}>
  {#if !items.length}<div class="none">ยังไม่มี AI ทำงาน — สตูดิโอว่างอยู่ จดไอเดียหรือสั่งงานในแชตแล้วจะมีคนมานั่งทำงานที่นี่</div>{/if}
  {#if tip}<div class="tip" style="left:{tip.x + 14}px;top:{tip.y + 10}px"><b>#{tip.id} {tip.title}</b>{#if tip.model}<div>{tip.model}</div>{/if}{#if tip.step}<div class="muted">{tip.step}</div>{/if}<div class="faint">คลิกเพื่อเปิดไอเดีย</div></div>{/if}
</div>

<style>
  .s3d { position: absolute; inset: 0; }
  .s3d :global(canvas) { display: block; width: 100%; height: 100%; }
  .none { position: absolute; left: 50%; top: 16px; transform: translateX(-50%); color: var(--text-3); font-size: 13px; background: var(--card); border: 1px solid var(--line);
    border-radius: 999px; padding: 6px 16px; z-index: 2; white-space: nowrap; }
  .tip { position: absolute; z-index: 3; pointer-events: none; background: var(--card); border: 1px solid var(--line-2); border-radius: 10px; padding: 8px 12px; font-size: 12.5px;
    box-shadow: 0 6px 18px rgba(0, 0, 0, .3); max-width: 280px; }
  .muted { color: var(--text-2); } .faint { color: var(--text-3); margin-top: 2px; }
</style>

#!/usr/bin/env python3
"""Collect every Thai piece of interface text that comes from the system (not from what the owner types).

Output: app/src/lib/i18n/source.json = sorted list of Thai *templates*; `{0}`, `{1}` mark a value filled in
at run time (a number, a name, ...). The same templates are matched against the page while it is displayed
(app/src/lib/uitrans.js), so a translation file maps each template to its text in another language.

Sources: markup text and attributes of .svelte files, string literals in .svelte/.js, and short single-line Thai
string literals of the Rust files that produce messages shown in the interface. Prompts for the AI and the rules
files are not interface text and are skipped.

Usage:  python scripts/extract-i18n.py            (rewrites source.json, prints what is new)
        python scripts/extract-i18n.py --check    (also lists templates missing from en.json)
"""
import json, re, sys, glob, os

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SRC = os.path.join(ROOT, "app", "src")
RUST = os.path.join(ROOT, "app", "src-tauri", "src")
OUT = os.path.join(SRC, "lib", "i18n")
THAI = re.compile(r"[฀-๿]")
SKIP_JS = ("i18n.js", "mock.js", "uitrans.js")
# Rust files whose Thai text is for the AI or for other channels, not for the interface
SKIP_RS = ("prompts.rs", "workspace.rs", "chat.rs", "agents.rs", "bin")

def norm(s):
    return re.sub(r"\s+", " ", s).strip()

def balanced(s, i):
    """index just after the {...} that starts at s[i]"""
    d = 0
    j = i
    while j < len(s):
        c = s[j]
        if c == "{": d += 1
        elif c == "}":
            d -= 1
            if d == 0: return j + 1
        elif c in "\"'`":
            q = c; j += 1
            while j < len(s) and s[j] != q:
                if s[j] == "\\": j += 1
                j += 1
        j += 1
    return len(s)

def js_strings(code):
    """Thai string / template literals inside a piece of JS or a {expression}."""
    out = []
    i = 0
    while i < len(code):
        c = code[i]
        if c in "\"'`":
            q = c; j = i + 1; buf = ""; n = 0
            while j < len(code) and code[j] != q:
                if code[j] == "\\" and j + 1 < len(code):
                    nx = code[j + 1]
                    buf += {"n": " ", "t": " "}.get(nx, nx); j += 2; continue
                if q == "`" and code[j] == "$" and j + 1 < len(code) and code[j + 1] == "{":
                    k = balanced(code, j + 1)
                    out += js_strings(code[j + 2:k - 1])
                    buf += "{%d}" % n; n += 1; j = k; continue
                buf += code[j]; j += 1
            if THAI.search(buf): out.append(buf)
            i = j + 1
        elif code.startswith("//", i):
            i = code.find("\n", i)
            if i < 0: break
        else:
            i += 1
    return out

def markup(text):
    """Thai text runs and attribute values of svelte markup (script/style removed)."""
    found = []
    i = 0
    run = ""
    n = 0
    def flush():
        nonlocal run, n
        if THAI.search(run): found.append(run)
        run = ""; n = 0
    while i < len(text):
        c = text[i]
        if c == "<":
            m = re.match(r"<!--.*?-->", text[i:], re.S)
            if m:
                i += len(m.group(0)); continue
            flush()
            j = i + 1
            # attributes
            while j < len(text) and text[j] != ">":
                if text[j] == "{":
                    k = balanced(text, j)
                    found += js_strings(text[j + 1:k - 1]); j = k; continue
                m = re.match(r'([\w:-]+)\s*=\s*"([^"]*)"', text[j:])
                if m:
                    val = m.group(2)
                    if "{" in val:
                        t = ""; p = 0; nn = 0
                        while p < len(val):
                            if val[p] == "{":
                                k = balanced(val, p); found += js_strings(val[p + 1:k - 1]); t += "{%d}" % nn; nn += 1; p = k
                            else:
                                t += val[p]; p += 1
                        if THAI.search(t): found.append(t)
                    elif THAI.search(val): found.append(val)
                    j += len(m.group(0)); continue
                j += 1
            i = j + 1
        elif c == "{":
            k = balanced(text, i)
            inner = text[i + 1:k - 1]
            if inner[:1] in "#:/@":
                flush()                      # {#if} {:else} {/each} end a run
                found += js_strings(inner)
            else:
                found += js_strings(inner)
                run += "{%d}" % n; n += 1
            i = k
        else:
            run += c; i += 1
    flush()
    return found

def svelte(path):
    s = open(path, encoding="utf-8").read()
    found = []
    for m in re.finditer(r"<script[^>]*>(.*?)</script>", s, re.S):
        found += js_strings(m.group(1))
    body = re.sub(r"<script[^>]*>.*?</script>|<style[^>]*>.*?</style>", "", s, flags=re.S)
    found += markup(body)
    return found

def rust(path):
    s = open(path, encoding="utf-8").read()
    found = []
    for m in re.finditer(r'"((?:[^"\\\n]|\\.)*)"', s):
        t = m.group(1)
        if not THAI.search(t) or len(t) > 160: continue
        t = t.replace('\\"', '"').replace("\\n", " ")
        n = [0]
        def sub(mm):
            v = "{%d}" % n[0]; n[0] += 1; return v
        t = re.sub(r"\{[^{}]*\}", sub, t)
        found.append(t)
    return found

def main():
    found = set()
    for f in glob.glob(os.path.join(SRC, "**", "*.svelte"), recursive=True):
        found.update(svelte(f))
    for f in glob.glob(os.path.join(SRC, "**", "*.js"), recursive=True):
        if os.path.basename(f) in SKIP_JS: continue
        found.update(js_strings(open(f, encoding="utf-8").read()))
    for f in glob.glob(os.path.join(RUST, "*.rs")):
        if os.path.basename(f) in SKIP_RS: continue
        found.update(rust(f))
    # the stage names of prompts.rs are interface text; the rest of that file is the AI's instructions
    found.update(m for m in re.findall(r'label: "([^"]*[\u0E00-\u0E7F][^"]*)"', open(os.path.join(RUST, "prompts.rs"), encoding="utf-8").read()))
    # a template with nothing but placeholders and punctuation is not text
    keep = sorted({norm(t) for t in found if len(THAI.findall(t)) >= 1 and norm(t)})
    os.makedirs(OUT, exist_ok=True)
    old = []
    p = os.path.join(OUT, "source.json")
    if os.path.exists(p):
        old = json.load(open(p, encoding="utf-8"))
    json.dump(keep, open(p, "w", encoding="utf-8"), ensure_ascii=False, indent=0)
    new = [t for t in keep if t not in set(old)]
    print(f"{len(keep)} templates ({len(new)} new)")
    if "--check" in sys.argv:
        en = json.load(open(os.path.join(OUT, "en.json"), encoding="utf-8")) if os.path.exists(os.path.join(OUT, "en.json")) else {}  # {thai template: english}
        missing = [t for t in keep if t not in en]
        print(f"{len(missing)} missing from en.json")
        for t in missing[:80]: print("  ", t)

if __name__ == "__main__":
    main()

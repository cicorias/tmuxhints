// Renders the cheat sheet supplied by the Rust side (`get_cheatsheet`).
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

function el(tag, className, text) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

// "Ctrl+b ( / )" -> [prefix chip] then [(] / [)]; shell commands render as code.
function renderKeys(keys, prefix) {
  const out = el("span", "keys");
  if (keys === "tmux" || keys.startsWith("tmux ")) {
    out.append(el("code", null, keys));
    return out;
  }
  let rest = keys;
  if (prefix && keys.startsWith(prefix + " ")) {
    out.append(el("kbd", "key key-prefix", prefix), el("span", "then", "then"));
    rest = keys.slice(prefix.length + 1);
  }
  const tokens = rest.split(" ").filter(Boolean);
  tokens.forEach((token, i) => {
    const isSeparator = token === "/" && i > 0 && i < tokens.length - 1;
    out.append(isSeparator ? el("span", "sep", "/") : el("kbd", "key", token));
  });
  return out;
}

function render(data) {
  document.getElementById("title").textContent = data.title || "tmux cheat sheet";
  document.getElementById("subtitle").textContent = data.subtitle || "";
  document.getElementById("prefix").textContent = data.prefix;
  document.getElementById("hotkey").textContent = data.hotkey;

  const warning = document.getElementById("warning");
  warning.hidden = !data.warning;
  warning.textContent = data.warning || "";

  const main = document.getElementById("sections");
  main.replaceChildren(
    ...data.sections.map((section) => {
      const node = el("section");
      node.append(el("h2", null, section.name));
      if (section.note) node.append(el("p", "note", section.note));
      for (const item of section.items) {
        const row = el("div", "row");
        row.append(renderKeys(item.keys, data.prefix), el("span", "desc", item.desc));
        node.append(row);
      }
      return node;
    }),
  );
}

async function load() {
  render(await invoke("get_cheatsheet"));
}

// Keyboard scrolling, vim/less style:
//   j k ↑ ↓            one line         Ctrl+d Ctrl+u     half page
//   Space Shift+Space  PgDn PgUp        Ctrl+f Ctrl+b     full page
//   gg Home / G End    top / bottom     Esc q             close
const scroller = document.querySelector("main");
const LINE = 48;
const GG_WINDOW_MS = 600;
let lastG = -Infinity; // time of a pending first `g`

function scrollByAmount(top, smooth = false) {
  scroller.scrollBy({ top, behavior: smooth ? "smooth" : "auto" });
}

function scrollToEdge(bottom) {
  scroller.scrollTo({ top: bottom ? scroller.scrollHeight : 0, behavior: "smooth" });
}

function keyAction(event) {
  const page = scroller.clientHeight * 0.9;
  const half = scroller.clientHeight / 2;

  if (event.ctrlKey) {
    return {
      d: () => scrollByAmount(half, true),
      u: () => scrollByAmount(-half, true),
      f: () => scrollByAmount(page, true),
      b: () => scrollByAmount(-page, true),
    }[event.key];
  }

  switch (event.key) {
    case "j":
    case "ArrowDown":
      return () => scrollByAmount(LINE);
    case "k":
    case "ArrowUp":
      return () => scrollByAmount(-LINE);
    case " ":
      return () => scrollByAmount(event.shiftKey ? -page : page, true);
    case "PageDown":
      return () => scrollByAmount(page, true);
    case "PageUp":
      return () => scrollByAmount(-page, true);
    case "Home":
      return () => scrollToEdge(false);
    case "End":
    case "G":
      return () => scrollToEdge(true);
    case "g":
      // `gg` goes to the top; a lone `g` just waits for the second one.
      if (event.timeStamp - lastG < GG_WINDOW_MS) {
        return () => {
          lastG = -Infinity;
          scrollToEdge(false);
        };
      }
      return () => (lastG = event.timeStamp);
    case "Escape":
    case "q":
      return () => invoke("hide_popup");
  }
}

document.addEventListener("keydown", (event) => {
  if (event.altKey || event.metaKey) return;
  const action = keyAction(event);
  if (event.key !== "g") lastG = -Infinity;
  if (!action) return;
  event.preventDefault();
  action();
});

listen("reload", load);
load();

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

document.addEventListener("keydown", (event) => {
  if (event.key === "Escape") invoke("hide_popup");
});

listen("reload", load);
load();

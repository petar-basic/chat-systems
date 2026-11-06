import { colorFor, initials, label, shortcutHint } from './labels.js';

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const list = document.getElementById('servers');

const OPEN_DELAY_MS = 400;
const WARM_FOR_MS = 300;
let hinted = null;
let pending = null;
let hiddenAt = 0;

function sendHint(hint) {
  invoke('rail_hint', { hint }).catch(() => {});
}

function showHint(button, animate) {
  clearTimeout(pending);
  pending = null;
  hinted = button;
  const rect = button.getBoundingClientRect();
  sendHint({ text: button.getAttribute('aria-label'), y: rect.top + rect.height / 2, animate });
}

function hideHint() {
  clearTimeout(pending);
  pending = null;
  if (!hinted) return;
  hinted = null;
  hiddenAt = Date.now();
  sendHint(null);
}

function hintOn(button) {
  button.addEventListener('pointerenter', (event) => {
    if (event.pointerType !== 'mouse') return;
    if (hinted || Date.now() - hiddenAt < WARM_FOR_MS) {
      showHint(button, false);
      return;
    }
    clearTimeout(pending);
    pending = setTimeout(() => showHint(button, true), OPEN_DELAY_MS);
  });
  button.addEventListener('pointerleave', hideHint);
  button.addEventListener('pointerdown', hideHint);
  button.addEventListener('focus', () => {
    if (button.matches(':focus-visible')) showHint(button, !hinted);
  });
  button.addEventListener('blur', hideHint);
}

function host(url) {
  return new URL(url).host;
}

function tile({ name, current, onClick, onMenu, image, text, color, className }) {
  const button = document.createElement('button');
  button.type = 'button';
  button.className = `server ${className ?? ''}`.trim();
  button.setAttribute('aria-label', name);
  hintOn(button);
  button.setAttribute('aria-current', String(current));
  const fallback = () => {
    if (color) button.style.setProperty('--tile', color);
    return document.createTextNode(text);
  };
  if (image) {
    const img = document.createElement('img');
    img.src = image;
    img.alt = '';
    img.addEventListener('error', () => img.replaceWith(fallback()));
    button.append(img);
  } else {
    button.append(fallback());
  }
  button.addEventListener('click', onClick);
  button.addEventListener('contextmenu', (event) => {
    event.preventDefault();
    onMenu();
  });
  return button;
}

function badge(count) {
  const el = document.createElement('span');
  el.className = 'badge';
  el.textContent = count > 99 ? '99+' : String(count);
  el.setAttribute('aria-label', `${count} unread`);
  return el;
}

function shortcut(index) {
  return shortcutHint(index, navigator.platform);
}

function workspaceItem(server, workspace, active, index) {
  const item = document.createElement('li');
  const current = active && server.current === workspace.id;
  item.append(
    tile({
      name: `${workspace.name}${workspace.deleted ? ' (deleted)' : ''}${shortcut(index)}`,
      current,
      image: workspace.icon,
      text: initials(workspace.name),
      color: colorFor(workspace.id),
      className: workspace.deleted ? 'deleted' : 'workspace',
      onClick: () => invoke('open_workspace', { url: server.url, workspaceId: workspace.id }),
      onMenu: () => invoke('rail_menu', { url: server.url, workspaceId: workspace.id }),
    }),
  );
  if (workspace.unread > 0 && !current) item.append(badge(workspace.unread));
  return item;
}

function serverItem(server, index, servers, active) {
  const item = document.createElement('li');
  item.append(
    tile({
      name: `${server.name} · ${host(server.url)}${shortcut(index)}`,
      current: server.url === active,
      image: server.icon,
      text: label(server, servers),
      onClick: () => invoke('switch_server', { url: server.url }),
      onMenu: () => invoke('rail_menu', { url: server.url }),
    }),
  );
  return item;
}

function entries(servers) {
  return servers.flatMap((server) =>
    server.workspaces.length === 0 ? [{ server }] : server.workspaces.map((workspace) => ({ server, workspace })),
  );
}

const PAINTED = [
  ['--rail', 'rail'],
  ['--line', 'line'],
  ['--fg', 'fg'],
  ['--accent', 'fg'],
  ['--muted', 'muted'],
  ['--badge', 'badge'],
  ['--badge-fg', 'badge_fg'],
];

function paint(colours) {
  const { style } = document.documentElement;
  for (const [property, key] of PAINTED) {
    if (colours) style.setProperty(property, colours[key]);
    else style.removeProperty(property);
  }
}

function render({ servers, active, theme, colours }) {
  if (theme) document.documentElement.dataset.theme = theme;
  else delete document.documentElement.dataset.theme;
  paint(colours);
  list.replaceChildren(
    ...entries(servers).map(({ server, workspace }, index) =>
      workspace
        ? workspaceItem(server, workspace, server.url === active, index)
        : serverItem(server, index, servers, active),
    ),
  );
  clearTimeout(pending);
  pending = null;
  if (!hinted) return;
  const name = hinted.getAttribute('aria-label');
  const same = [...list.querySelectorAll('button')].find((button) => button.getAttribute('aria-label') === name);
  if (same) hinted = same;
  else hideHint();
}

const add = document.getElementById('add');
add.addEventListener('click', () => invoke('add_menu'));
hintOn(add);
document.querySelector('nav').addEventListener('scroll', hideHint);
window.addEventListener('blur', hideHint);
listen('servers-changed', (event) => render(event.payload));
invoke('list_servers').then(render);

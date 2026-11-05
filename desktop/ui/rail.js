import { colorFor, initials, label, shortcutHint } from './labels.js';

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const list = document.getElementById('servers');

function host(url) {
  return new URL(url).host;
}

function tile({ title, current, onClick, onMenu, image, text, color, className }) {
  const button = document.createElement('button');
  button.type = 'button';
  button.className = `server ${className ?? ''}`.trim();
  button.title = title;
  button.setAttribute('aria-label', title);
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
      title: `${workspace.name}${workspace.deleted ? ' (deleted)' : ''} · ${server.name}${shortcut(index)}`,
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
      title: `${server.name} · ${host(server.url)}${shortcut(index)}`,
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
}

document.getElementById('add').addEventListener('click', () => invoke('add_menu'));
listen('servers-changed', (event) => render(event.payload));
invoke('list_servers').then(render);

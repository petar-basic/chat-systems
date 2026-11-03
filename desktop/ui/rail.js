import { colorFor, initials, label } from './labels.js';

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
  if (color) button.style.setProperty('--tile', color);
  const fallback = () => document.createTextNode(text);
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

function workspaceItem(server, workspace, active) {
  const item = document.createElement('li');
  const current = active && server.current === workspace.id;
  item.append(
    tile({
      title: `${workspace.name}${workspace.deleted ? ' (deleted)' : ''} · ${server.name}`,
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
      title: `${server.name} · ${host(server.url)}${index < 9 ? ` (⌘${index + 1})` : ''}`,
      current: server.url === active,
      image: server.icon,
      text: label(server, servers),
      onClick: () => invoke('switch_server', { url: server.url }),
      onMenu: () => invoke('rail_menu', { url: server.url }),
    }),
  );
  return item;
}

function items(server, index, servers, active) {
  if (server.workspaces.length === 0) return [serverItem(server, index, servers, active)];
  return server.workspaces.map((workspace) => workspaceItem(server, workspace, server.url === active));
}

function render({ servers, active, theme }) {
  if (theme) document.documentElement.dataset.theme = theme;
  else delete document.documentElement.dataset.theme;
  list.replaceChildren(...servers.flatMap((server, i) => items(server, i, servers, active)));
}

document.getElementById('add').addEventListener('click', () => invoke('add_menu'));
listen('servers-changed', (event) => render(event.payload));
invoke('list_servers').then(render);

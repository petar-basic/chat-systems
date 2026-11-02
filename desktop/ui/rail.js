import { colorFor, initials, label } from './labels.js';

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const list = document.getElementById('servers');

function host(url) {
  return new URL(url).host;
}

function tile({ title, current, onClick, image, text, color, className }) {
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
      title: `${workspace.name}${workspace.deleted ? ' (deleted)' : ''} · ${host(server.url)}`,
      current,
      image: workspace.icon,
      text: initials(workspace.name),
      color: colorFor(workspace.id),
      className: workspace.deleted ? 'deleted' : 'workspace',
      onClick: () => invoke('open_workspace', { url: server.url, workspaceId: workspace.id }),
    }),
  );
  if (workspace.unread > 0 && !current) item.append(badge(workspace.unread));
  return item;
}

function group(server, index, servers, active) {
  const items = [];
  const isActive = server.url === active;
  if (index > 0) {
    const divider = document.createElement('li');
    divider.className = 'divider';
    divider.setAttribute('role', 'separator');
    items.push(divider);
  }
  if (server.workspaces.length === 0) {
    const item = document.createElement('li');
    item.append(
      tile({
        title: `${server.name} · ${host(server.url)}${index < 9 ? ` (⌘${index + 1})` : ''}`,
        current: isActive,
        image: server.icon,
        text: label(server, servers),
        onClick: () => invoke('switch_server', { url: server.url }),
      }),
    );
    items.push(item);
    return items;
  }
  for (const workspace of server.workspaces) items.push(workspaceItem(server, workspace, isActive));
  const create = document.createElement('li');
  create.append(
    tile({
      title: `Create a workspace on ${host(server.url)}`,
      current: false,
      text: '+',
      className: 'create',
      onClick: () => invoke('create_workspace', { url: server.url }),
    }),
  );
  items.push(create);
  return items;
}

function render({ servers, active }) {
  list.replaceChildren(...servers.flatMap((server, i) => group(server, i, servers, active)));
}

document.getElementById('add').addEventListener('click', () => invoke('show_add_server'));
listen('servers-changed', (event) => render(event.payload));
invoke('list_servers').then(render);

import { label } from './labels.js';

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const list = document.getElementById('servers');

function render({ servers, active }) {
  list.replaceChildren(
    ...servers.map((server, i) => {
      const item = document.createElement('li');
      const button = document.createElement('button');
      button.type = 'button';
      button.className = 'server';
      button.title = `${server.name} — ${server.url}${i < 9 ? ` (⌘${i + 1})` : ''}`;
      button.setAttribute('aria-label', server.name);
      button.setAttribute('aria-current', String(server.url === active));
      if (server.icon) {
        const img = document.createElement('img');
        img.src = server.icon;
        img.alt = '';
        img.addEventListener('error', () => img.replaceWith(label(server, servers)));
        button.append(img);
      } else {
        button.append(label(server, servers));
      }
      button.addEventListener('click', () => invoke('switch_server', { url: server.url }));
      item.append(button);
      return item;
    }),
  );
}

document.getElementById('add').addEventListener('click', () => invoke('show_add_server'));
listen('servers-changed', (event) => render(event.payload));
invoke('list_servers').then(render);

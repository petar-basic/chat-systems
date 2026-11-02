const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const form = document.getElementById('add');
const address = document.getElementById('address');
const error = document.getElementById('error');
const submit = form.querySelector('button[type="submit"]');
const saved = document.getElementById('saved');
const list = document.getElementById('servers');

function showError(message) {
  error.textContent = message;
  error.hidden = !message;
}

function button(text, className, onClick) {
  const el = document.createElement('button');
  el.type = 'button';
  el.textContent = text;
  if (className) el.className = className;
  el.addEventListener('click', onClick);
  return el;
}

function row(server) {
  const item = document.createElement('li');
  const who = document.createElement('div');
  who.className = 'who';
  const name = document.createElement('div');
  name.className = 'name';
  name.textContent = server.name;
  const url = document.createElement('div');
  url.className = 'address';
  url.textContent = server.url;
  who.append(name, url);

  const remove = button('Remove', 'remove', async () => {
    if (remove.dataset.armed !== 'true') {
      remove.dataset.armed = 'true';
      remove.textContent = 'Remove?';
      return;
    }
    await invoke('remove_server', { url: server.url }).catch((e) => showError(String(e)));
  });
  remove.addEventListener('blur', () => {
    remove.dataset.armed = 'false';
    remove.textContent = 'Remove';
  });

  const open = button('Open', '', () =>
    invoke('switch_server', { url: server.url }).catch((e) => showError(String(e))),
  );
  item.append(who, open, remove);
  return item;
}

function render({ servers }) {
  list.replaceChildren(...servers.map(row));
  saved.hidden = servers.length === 0;
}

form.addEventListener('submit', async (event) => {
  event.preventDefault();
  showError('');
  submit.disabled = true;
  submit.textContent = 'Connecting…';
  const [result] = await Promise.allSettled([
    invoke('add_server', { address: address.value }),
    new Promise((resolve) => setTimeout(resolve, 400)),
  ]);
  submit.disabled = false;
  submit.textContent = 'Connect';
  if (result.status === 'fulfilled') {
    address.value = '';
    return;
  }
  showError(String(result.reason));
  address.focus();
  address.select();
});

listen('servers-changed', (event) => render(event.payload));
invoke('list_servers').then(render);

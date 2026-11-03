import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import './index.css';
import './lib/i18n';
import './stores/theme';
import App from './App.tsx';
import { reloadWhenChunksAreGone } from './lib/chunkReload';

reloadWhenChunksAreGone();

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);

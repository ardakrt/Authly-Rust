import '@fontsource/space-grotesk/500.css';
import '@fontsource/space-grotesk/600.css';
import '@fontsource/space-grotesk/700.css';
import '@fontsource/ibm-plex-sans/400.css';
import '@fontsource/ibm-plex-sans/500.css';
import '@fontsource/ibm-plex-sans/600.css';
import '@fontsource/jetbrains-mono/400.css';
import '@fontsource/jetbrains-mono/500.css';
import '../../../tokens.css';
import './styles/app.css';

import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { HashRouter } from 'react-router-dom';
import { App } from './app/App';
import { LanguageProvider } from './app/LanguageProvider';
import { ThemeProvider } from './app/ThemeProvider';
import { installTauriBridge } from './bridge/tauriBridge';

installTauriBridge();

const root = document.getElementById('root');
if (!root) throw new Error('Uygulama arayüzü başlatılamadı.');

createRoot(root).render(
  <StrictMode>
    <HashRouter>
      <ThemeProvider>
        <LanguageProvider>
          <App />
        </LanguageProvider>
      </ThemeProvider>
    </HashRouter>
  </StrictMode>,
);

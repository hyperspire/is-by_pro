const APP_PANELS = ['home', 'mission', 'access'];
const SHELL_CACHE_NAME = 'is-by-mobile-shell-v1';

let deferredInstallPrompt = null;

function setStatus(message) {
  const status = document.getElementById('connection-status');
  if (status) {
    status.textContent = message;
  }
}

function setInstallStatus(message) {
  const installStatus = document.getElementById('install-status');
  if (installStatus) {
    installStatus.textContent = message;
  }
}

function activatePanel(panelName) {
  const nextPanel = APP_PANELS.includes(panelName) ? panelName : 'home';

  document.querySelectorAll('[data-panel]').forEach((panel) => {
    panel.classList.toggle('is-active', panel.dataset.panel === nextPanel);
  });

  document.querySelectorAll('.pwa-nav-item').forEach((button) => {
    button.classList.toggle('is-active', button.dataset.target === nextPanel);
  });

  if (window.location.hash !== `#${nextPanel}`) {
    history.replaceState(null, '', `#${nextPanel}`);
  }
}

function handleHashRoute() {
  const hash = window.location.hash.replace('#', '').trim();
  activatePanel(hash || 'home');
}

function bindPanelNavigation() {
  document.querySelectorAll('[data-target]').forEach((element) => {
    element.addEventListener('click', (event) => {
      const target = element.dataset.target;
      if (!APP_PANELS.includes(target)) {
        return;
      }

      event.preventDefault();
      activatePanel(target);
      window.scrollTo({ top: 0, behavior: 'smooth' });
    });
  });
}

async function registerServiceWorker() {
  if (!('serviceWorker' in navigator)) {
    setStatus('PWA install is unavailable in this browser.');
    return;
  }

  try {
    const registration = await navigator.serviceWorker.register('/sw.js', { scope: '/' });
    setStatus(`Mobile shell cached: ${SHELL_CACHE_NAME}`);

    if (registration.waiting) {
      setInstallStatus('An updated shell is ready. Reload the page to apply it.');
    }
  } catch (error) {
    console.error('Service worker registration failed', error);
    setStatus('Service worker registration failed.');
  }
}

function bindInstallPrompt() {
  const installButton = document.getElementById('install-app-button');
  if (!installButton) {
    return;
  }

  window.addEventListener('beforeinstallprompt', (event) => {
    event.preventDefault();
    deferredInstallPrompt = event;
    installButton.hidden = false;
    setInstallStatus('Install is-by.pro for faster access and offline shell support.');
  });

  window.addEventListener('appinstalled', () => {
    deferredInstallPrompt = null;
    installButton.hidden = true;
    setInstallStatus('is-by.pro is installed on this device.');
  });

  installButton.addEventListener('click', async () => {
    if (!deferredInstallPrompt) {
      setInstallStatus('Use your browser menu to add this app to the home screen.');
      return;
    }

    deferredInstallPrompt.prompt();
    const choice = await deferredInstallPrompt.userChoice;
    deferredInstallPrompt = null;
    installButton.hidden = true;

    if (choice.outcome === 'accepted') {
      setInstallStatus('Installation accepted. Launch the app from your home screen.');
    } else {
      setInstallStatus('Installation dismissed. You can install it later from the browser menu.');
    }
  });
}

function bindConnectivityState() {
  const update = () => {
    if (navigator.onLine) {
      setStatus('Online and ready');
    } else {
      setStatus('Offline mode: cached shell active');
    }
  };

  window.addEventListener('online', update);
  window.addEventListener('offline', update);
  update();
}

document.addEventListener('DOMContentLoaded', () => {
  bindPanelNavigation();
  bindInstallPrompt();
  bindConnectivityState();
  handleHashRoute();
  window.addEventListener('hashchange', handleHashRoute);
  registerServiceWorker();
});

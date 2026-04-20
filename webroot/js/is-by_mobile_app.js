const APP_PANELS = ['home', 'mission', 'access'];
const SHELL_CACHE_NAME = 'is-by-mobile-shell-v2';

let deferredInstallPrompt = null;

function isIosDevice() {
  return /iphone|ipad|ipod/i.test(window.navigator.userAgent || '');
}

function isStandaloneDisplay() {
  return window.matchMedia('(display-mode: standalone)').matches || window.navigator.standalone === true;
}

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

function setInstallGuide(message, visible = true) {
  const installGuide = document.getElementById('install-guide');
  const installGuideText = document.getElementById('install-guide-text');
  if (!installGuide || !installGuideText) {
    return;
  }

  installGuideText.textContent = message;
  installGuide.hidden = !visible;
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

  if (isStandaloneDisplay()) {
    installButton.hidden = true;
    setInstallStatus('is-by.pro is already installed on this device.');
    setInstallGuide('', false);
    return;
  }

  if (isIosDevice()) {
    installButton.hidden = false;
    installButton.textContent = 'How to Install';
    setInstallStatus('Safari does not show a PWA prompt. Use the share menu to install.');
    setInstallGuide('In Safari, tap Share, then choose Add to Home Screen.', true);
  } else {
    installButton.hidden = false;
    installButton.textContent = 'Install App';
    setInstallGuide('If no install prompt appears, open the browser menu and choose Install App or Add to Home Screen.', true);
  }

  window.addEventListener('beforeinstallprompt', (event) => {
    event.preventDefault();
    deferredInstallPrompt = event;
    installButton.hidden = false;
    installButton.textContent = 'Install App';
    setInstallStatus('Install is-by.pro for faster access and offline shell support.');
    setInstallGuide('Tap Install App to add is-by.pro to your home screen.', true);
  });

  window.addEventListener('appinstalled', () => {
    deferredInstallPrompt = null;
    installButton.hidden = true;
    setInstallStatus('is-by.pro is installed on this device.');
    setInstallGuide('', false);
  });

  installButton.addEventListener('click', async () => {
    if (isIosDevice() && !deferredInstallPrompt) {
      setInstallStatus('Use Safari Share > Add to Home Screen to install is-by.pro.');
      setInstallGuide('In Safari, tap Share, then choose Add to Home Screen.', true);
      return;
    }

    if (!deferredInstallPrompt) {
      setInstallStatus('Use your browser menu to add this app to the home screen.');
      setInstallGuide('Open the browser menu and choose Install App or Add to Home Screen.', true);
      return;
    }

    deferredInstallPrompt.prompt();
    const choice = await deferredInstallPrompt.userChoice;
    deferredInstallPrompt = null;
    installButton.hidden = true;

    if (choice.outcome === 'accepted') {
      setInstallStatus('Installation accepted. Launch the app from your home screen.');
      setInstallGuide('', false);
    } else {
      setInstallStatus('Installation dismissed. You can install it later from the browser menu.');
      setInstallGuide('Open the browser menu and choose Install App or Add to Home Screen when you are ready.', true);
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

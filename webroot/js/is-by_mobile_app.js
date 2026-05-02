const APP_PANELS = ['home', 'mission', 'access'];
const SHELL_CACHE_NAME = 'is-by-mobile-shell-v9';
const SHELL_VERSION = 'mobile-shell-v9';
const UPDATE_BANNER_SUPPRESS_KEY = 'is-by-mobile-update-banner-suppressed';
const UPDATE_BANNER_SUPPRESS_VERSION_KEY = 'is-by-mobile-update-banner-suppressed-version';

let deferredInstallPrompt = null;
let refreshingForUpdate = false;

window.addEventListener('beforeinstallprompt', (event) => {
  console.log('beforeinstallprompt event triggered');
  event.preventDefault();
  deferredInstallPrompt = event;

  const installButton = document.getElementById('install-app-button');
  if (installButton) {
    installButton.hidden = false;
    installButton.textContent = 'Install App';

    const installStatus = document.getElementById('install-status');
    if (installStatus) installStatus.textContent = 'Install is-by.pro for faster access and offline shell support.';

    const installGuide = document.getElementById('install-guide');
    const installGuideText = document.getElementById('install-guide-text');
    if (installGuide && installGuideText) {
      installGuideText.textContent = 'Tap Install App to add is-by.pro to your home screen.';
      installGuide.hidden = false;
    }
  }
});

function isFirefox() {
  return /firefox/i.test(window.navigator.userAgent || '');
}

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

function getSuppressedWorkerUrl() {
  try {
    return window.sessionStorage.getItem(UPDATE_BANNER_SUPPRESS_KEY) || '';
  } catch (_error) {
    return '';
  }
}

function clearUpdateBannerSuppression() {
  try {
    window.sessionStorage.removeItem(UPDATE_BANNER_SUPPRESS_KEY);
  } catch (_error) {
    // Ignore storage exceptions in strict/private modes.
  }
}

function suppressUpdateBannerForCurrentShellVersion() {
  try {
    window.sessionStorage.setItem(UPDATE_BANNER_SUPPRESS_VERSION_KEY, SHELL_VERSION);
  } catch (_error) {
    // Ignore storage exceptions in strict/private modes.
  }

  try {
    window.localStorage.setItem(UPDATE_BANNER_SUPPRESS_VERSION_KEY, SHELL_VERSION);
  } catch (_error) {
    // Ignore storage exceptions in strict/private modes.
  }
}

function isUpdateBannerSuppressedForCurrentShellVersion() {
  try {
    if (window.localStorage.getItem(UPDATE_BANNER_SUPPRESS_VERSION_KEY) === SHELL_VERSION) {
      return true;
    }
  } catch (_error) {
    // Ignore storage exceptions in strict/private modes.
  }

  try {
    return window.sessionStorage.getItem(UPDATE_BANNER_SUPPRESS_VERSION_KEY) === SHELL_VERSION;
  } catch (_error) {
    return false;
  }
}

function suppressUpdateBannerForSession(registration) {
  const waitingUrl = registration?.waiting?.scriptURL || '';
  if (!waitingUrl) {
    return;
  }

  try {
    window.sessionStorage.setItem(UPDATE_BANNER_SUPPRESS_KEY, waitingUrl);
  } catch (_error) {
    // Ignore storage exceptions in strict/private modes.
  }
}

function reconcileUpdateBannerSuppression(registration) {
  const waitingUrl = registration?.waiting?.scriptURL || '';
  if (!waitingUrl) {
    return;
  }

  const suppressedUrl = getSuppressedWorkerUrl();
  if (suppressedUrl && suppressedUrl !== waitingUrl) {
    clearUpdateBannerSuppression();
  }
}

function isUpdateBannerSuppressedForSession(registration) {
  const waitingUrl = registration?.waiting?.scriptURL || '';
  const suppressedUrl = getSuppressedWorkerUrl();
  return Boolean(waitingUrl && suppressedUrl && waitingUrl === suppressedUrl);
}

function suppressUpdateBannerForReloadCycle() {
  try {
    window.sessionStorage.setItem(`${UPDATE_BANNER_SUPPRESS_KEY}:reloading`, '1');
  } catch (_error) {
    // Ignore storage exceptions in strict/private modes.
  }
}

function clearReloadCycleSuppression() {
  try {
    window.sessionStorage.removeItem(`${UPDATE_BANNER_SUPPRESS_KEY}:reloading`);
  } catch (_error) {
    // Ignore storage exceptions in strict/private modes.
  }
}

function isReloadCycleSuppressed() {
  try {
    return window.sessionStorage.getItem(`${UPDATE_BANNER_SUPPRESS_KEY}:reloading`) === '1';
  } catch (_error) {
    return false;
  }
}

function showUpdateBanner(registration) {
  const updateBanner = document.getElementById('update-banner');
  const updateButton = document.getElementById('update-banner-action');
  reconcileUpdateBannerSuppression(registration);
  if (!updateBanner || !updateButton || !registration?.waiting || isUpdateBannerSuppressedForSession(registration) || isUpdateBannerSuppressedForCurrentShellVersion()) {
    return;
  }

  updateBanner.hidden = false;

  // Remove any existing listeners
  const newButton = updateButton.cloneNode(true);
  updateButton.parentNode.replaceChild(newButton, updateButton);

  // Attach fresh listener
  const freshButton = document.getElementById('update-banner-action');
  if (freshButton) {
    freshButton.addEventListener('click', () => {
      // Immediately hide the banner and button on click
      const updateBanner = document.getElementById('update-banner');
      freshButton.disabled = true;
      freshButton.textContent = 'Refreshing...';
      if (updateBanner) {
        updateBanner.hidden = true;
      }
      // Optionally, still do the SW update logic
      suppressUpdateBannerForCurrentShellVersion();
      if (registration.waiting) {
        suppressUpdateBannerForSession(registration);
        suppressUpdateBannerForReloadCycle();
        console.log('Sending SKIP_WAITING message to service worker');
        registration.waiting.postMessage({ type: 'SKIP_WAITING' });
      }
    });
  }
}

function hasNewWaitingWorker(registration) {
  if (!registration || !registration.waiting) {
    return false;
  }

  const waitingUrl = registration.waiting.scriptURL || '';
  const activeUrl = registration.active?.scriptURL || '';

  // Show the banner only when the waiting worker is different from active.
  if (activeUrl && waitingUrl === activeUrl) {
    return false;
  }

  return true;
}

function hideUpdateBanner() {
  const updateBanner = document.getElementById('update-banner');
  const updateButton = document.getElementById('update-banner-action');
  if (updateBanner) {
    updateBanner.hidden = true;
  }
  if (updateButton) {
    updateButton.disabled = false;
    updateButton.textContent = 'Refresh Now';
  }
}

function watchServiceWorkerRegistration(registration) {
  reconcileUpdateBannerSuppression(registration);

  if (hasNewWaitingWorker(registration)) {
    if (isUpdateBannerSuppressedForSession(registration) || isUpdateBannerSuppressedForCurrentShellVersion()) {
      hideUpdateBanner();
    } else {
      setInstallStatus('An updated shell is ready. Reload the page to apply it.');
      showUpdateBanner(registration);
    }
  } else {
    hideUpdateBanner();
  }

  registration.addEventListener('updatefound', () => {
    const installingWorker = registration.installing;
    if (!installingWorker) {
      return;
    }

    installingWorker.addEventListener('statechange', () => {
      if (installingWorker.state === 'installed' && navigator.serviceWorker.controller && hasNewWaitingWorker(registration)) {
        reconcileUpdateBannerSuppression(registration);
        if (isUpdateBannerSuppressedForSession(registration) || isUpdateBannerSuppressedForCurrentShellVersion()) {
          hideUpdateBanner();
        } else {
          setInstallStatus('An updated shell is ready. Refresh the page to apply it.');
          showUpdateBanner(registration);
        }
      } else if (installingWorker.state === 'activated') {
        clearReloadCycleSuppression();
        hideUpdateBanner();
      }
    });
  });
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
    const registration = await navigator.serviceWorker.register(`/sw.js?v=${SHELL_VERSION}`, { scope: '/' });
    registration.update();
    setStatus(`Mobile shell cached: ${SHELL_CACHE_NAME}`);
    watchServiceWorkerRegistration(registration);

    navigator.serviceWorker.addEventListener('controllerchange', () => {
      hideUpdateBanner();
      if (refreshingForUpdate) {
        return;
      }
      refreshingForUpdate = true;
      clearReloadCycleSuppression();
      window.location.reload();
    });

    // Initialize push notifications after SW is ready
    initPushNotifications(registration);
  } catch (error) {
    console.error('Service worker registration failed', error);
    setStatus('Service worker registration failed.');
  }
}

function urlBase64ToUint8Array(base64String) {
  const padding = '='.repeat((4 - base64String.length % 4) % 4);
  const base64 = (base64String + padding)
    .replace(/\-/g, '+')
    .replace(/_/g, '/');

  const rawData = window.atob(base64);
  const outputArray = new Uint8Array(rawData.length);

  for (let i = 0; i < rawData.length; ++i) {
    outputArray[i] = rawData.charCodeAt(i);
  }
  return outputArray;
}

async function initPushNotifications(registration) {
  try {
    const permission = await Notification.requestPermission();
    if (permission !== 'granted') {
      console.log('Push notification permission denied.');
      return;
    }

    const keyResponse = await fetch('/v1/push/public-key');
    if (!keyResponse.ok) {
      console.error('Failed to fetch VAPID public key');
      return;
    }
    const vapidPublicKey = await keyResponse.text();
    const applicationServerKey = urlBase64ToUint8Array(vapidPublicKey);

    const subscription = await registration.pushManager.subscribe({
      userVisibleOnly: true,
      applicationServerKey: applicationServerKey
    });

    const subJson = subscription.toJSON();
    const payload = {
      endpoint: subJson.endpoint,
      keys: {
        p256dh: subJson.keys.p256dh,
        auth: subJson.keys.auth
      }
    };

    const subscribeResponse = await fetch('/v1/push/subscribe', {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json'
      },
      body: JSON.stringify(payload)
    });

    if (subscribeResponse.ok) {
      console.log('Push subscription saved successfully.');
    } else {
      console.error('Failed to save push subscription to backend.');
    }
  } catch (error) {
    console.error('Failed to initialize push notifications:', error);
  }
}

function bindInstallPrompt() {
  const installButton = document.getElementById('install-app-button');
  if (!installButton) {
    console.warn('Install button not found in DOM');
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
  } else if (isFirefox()) {
    installButton.hidden = false;
    installButton.textContent = 'How to Install';
    setInstallStatus('Firefox does not support the install prompt.');
    setInstallGuide('Open the browser menu and choose "Install" or "Add to Home screen".', true);
  } else {
    installButton.hidden = false;
    installButton.textContent = 'Install App';
    setInstallGuide('If no install prompt appears, open the browser menu and choose Install App or Add to Home Screen.', true);
  }

  if (!isFirefox() && deferredInstallPrompt) {
    installButton.hidden = false;
    installButton.textContent = 'Install App';
    setInstallStatus('Install is-by.pro for faster access and offline shell support.');
    setInstallGuide('Tap Install App to add is-by.pro to your home screen.', true);
  }

  window.addEventListener('appinstalled', () => {
    console.log('appinstalled event triggered');
    deferredInstallPrompt = null;
    installButton.hidden = true;
    setInstallStatus('is-by.pro is installed on this device.');
    setInstallGuide('', false);
  });

  installButton.addEventListener('click', async (event) => {
    console.log('Install button clicked');
    event.preventDefault();
    event.stopPropagation();

    if (isIosDevice() && !deferredInstallPrompt) {
      installButton.textContent = 'Use Share Menu';
      setInstallStatus('Use Safari Share > Add to Home Screen to install is-by.pro.');
      setInstallGuide('In Safari, tap Share, then choose Add to Home Screen.', true);
      setTimeout(() => {
        installButton.textContent = 'How to Install';
      }, 2200);
      return;
    }

    if (isFirefox()) {
      installButton.textContent = 'Use Browser Menu';
      setInstallStatus('Use the Firefox menu to add this app to your home screen.');
      setInstallGuide('Open the browser menu and choose "Install" or "Add to Home screen".', true);
      setTimeout(() => {
        installButton.textContent = 'How to Install';
      }, 2200);
      return;
    }

    if (!deferredInstallPrompt) {
      installButton.textContent = 'Use Browser Menu';
      setInstallStatus('Use your browser menu to add this app to the home screen.');
      setInstallGuide('Open the browser menu and choose Install App or Add to Home Screen.', true);
      setTimeout(() => {
        installButton.textContent = 'Install App';
      }, 2200);
      return;
    }

    try {
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
    } catch (error) {
      console.error('Error during install prompt:', error);
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

function bindRefreshFallback() {
  const updateButton = document.getElementById('update-banner-action');
  if (!updateButton) {
    return;
  }

  updateButton.addEventListener('click', async () => {
    if (updateButton.disabled) {
      return;
    }
    // Immediately hide the banner and button on click
    const updateBanner = document.getElementById('update-banner');
    updateButton.disabled = true;
    updateButton.textContent = 'Refreshing...';
    if (updateBanner) {
      updateBanner.hidden = true;
    }
    suppressUpdateBannerForCurrentShellVersion();
    let didUpdate = false;
    try {
      const registration = await navigator.serviceWorker.getRegistration('/');
      if (registration) {
        await registration.update();
        if (registration.waiting) {
          suppressUpdateBannerForSession(registration);
          suppressUpdateBannerForReloadCycle();
          registration.waiting.postMessage({ type: 'SKIP_WAITING' });
          didUpdate = true;
          // Give the SW a moment to activate, then reload
          setTimeout(() => window.location.reload(), 1200);
          return;
        }
      }
      // Fallback when no waiting worker exists: force a fresh page load.
      window.location.reload();
    } catch (error) {
      console.error('Refresh fallback failed:', error);
      window.location.reload();
    }
    // If SW update did not trigger, reload after a short delay as fallback
    if (!didUpdate) {
      setTimeout(() => window.location.reload(), 1200);
    }
  });
}

document.addEventListener('DOMContentLoaded', () => {
  if (isReloadCycleSuppressed()) {
    hideUpdateBanner();
  }
  hideUpdateBanner();
  bindRefreshFallback();
  bindPanelNavigation();
  bindInstallPrompt();
  bindConnectivityState();
  handleHashRoute();
  window.addEventListener('hashchange', handleHashRoute);
  registerServiceWorker();
});

const SHELL_CACHE = 'is-by-mobile-shell-v3';
const SHELL_ASSETS = [
  '/mobile.html',
  '/css/is-by_mobile.css?v=mobile-shell-v3',
  '/js/is-by_mobile_app.js?v=mobile-shell-v3',
  '/images/Death_Angel-555x222.png',
  '/images/is-by_app_icon.svg',
  '/images/is-by_app_icon-192.png',
  '/images/is-by_app_icon-512.png',
  '/favicon.ico',
  '/app.webmanifest?v=mobile-shell-v3'
];

self.addEventListener('install', (event) => {
  event.waitUntil(
    caches.open(SHELL_CACHE).then((cache) => cache.addAll(SHELL_ASSETS)).then(() => self.skipWaiting())
  );
});

self.addEventListener('activate', (event) => {
  event.waitUntil(
    caches.keys().then((keys) => Promise.all(
      keys.filter((key) => key !== SHELL_CACHE).map((key) => caches.delete(key))
    )).then(() => self.clients.claim())
  );
});

self.addEventListener('message', (event) => {
  if (event.data && event.data.type === 'SKIP_WAITING') {
    self.skipWaiting();
  }
});

self.addEventListener('fetch', (event) => {
  const { request } = event;
  if (request.method !== 'GET') {
    return;
  }

  const url = new URL(request.url);
  if (url.origin !== self.location.origin) {
    return;
  }

  if (request.mode === 'navigate') {
    event.respondWith(
      fetch(request)
        .then((response) => {
          const copy = response.clone();
          caches.open(SHELL_CACHE).then((cache) => cache.put('/mobile.html', copy));
          return response;
        })
        .catch(() => caches.match('/mobile.html'))
    );
    return;
  }

  event.respondWith(
    caches.match(request).then((cached) => {
      if (cached) {
        return cached;
      }

      return fetch(request).then((response) => {
        const copy = response.clone();
        caches.open(SHELL_CACHE).then((cache) => cache.put(request, copy));
        return response;
      });
    })
  );
});

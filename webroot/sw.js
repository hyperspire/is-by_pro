const SHELL_CACHE = 'is-by-mobile-shell-v9';
const SHELL_ASSETS = [
  '/mobile.html',
  '/css/is-by_mobile.css?v=mobile-shell-v9',
  '/js/is-by_mobile_app.js?v=mobile-shell-v9',
  '/images/Death_Angel-555x222.png',
  '/images/is-by_app_icon.svg',
  '/images/is-by_app_icon-192.png?v=mobile-shell-v9',
  '/images/is-by_app_icon-512.png?v=mobile-shell-v9',
  '/favicon.ico',
  '/app.webmanifest?v=mobile-shell-v9'
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

  // Do not aggressively cache API routes
  if (url.pathname.startsWith('/v1/') || url.pathname.startsWith('/api/')) {
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

self.addEventListener('push', (event) => {
  if (event.data) {
    try {
      const data = event.data.json();
      const title = data.title || 'New Notification';
      const options = {
        body: data.body || '',
        icon: '/images/is-by_app_icon-192.png',
        badge: '/images/is-by_app_icon-192.png',
        vibrate: [200, 100, 200],
        data: {
          url: data.url || '/'
        }
      };

      event.waitUntil(self.registration.showNotification(title, options));
    } catch (e) {
      console.error('Push event payload parsing failed:', e);
    }
  }
});

self.addEventListener('notificationclick', (event) => {
  event.notification.close();
  if (event.notification.data && event.notification.data.url) {
    event.waitUntil(
      clients.matchAll({ type: 'window' }).then((clientList) => {
        for (const client of clientList) {
          if (client.url.includes(event.notification.data.url) && 'focus' in client) {
            return client.focus();
          }
        }
        if (clients.openWindow) {
          return clients.openWindow(event.notification.data.url);
        }
      })
    );
  }
});

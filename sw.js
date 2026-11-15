/* 賽前預測台 service worker：收 Web Push → 出通知 → 點擊開頁 */
const VER = 'md-sw-v4';
self.addEventListener('install', () => self.skipWaiting());
self.addEventListener('activate', (e) => e.waitUntil(self.clients.claim()));

self.addEventListener('push', (e) => {
  let d = { title: 'MatchDeck', body: '', url: './#watch' };
  try { if (e.data) Object.assign(d, e.data.json()); }
  catch (_) { if (e.data) d.body = e.data.text(); }
  e.waitUntil(self.registration.showNotification(d.title || 'MatchDeck', {
    body: d.body || '',
    tag: d.tag || '',
    data: { url: d.url || './#watch' },
    icon: 'doge-192.png',
    badge: 'badge.png?v=2',
    renotify: !!d.tag,
    vibrate: [60, 40, 60],
  }));
});

self.addEventListener('notificationclick', (e) => {
  e.notification.close();
  const url = (e.notification.data && e.notification.data.url) || './#watch';
  e.waitUntil((async () => {
    const cs = await self.clients.matchAll({ type: 'window', includeUncontrolled: true });
    for (const c of cs) {
      if ('focus' in c) { try { await c.navigate(url); } catch (_) {} return c.focus(); }
    }
    return self.clients.openWindow(url);
  })());
});

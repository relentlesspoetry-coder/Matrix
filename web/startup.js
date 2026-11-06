/* Keep a visible explanation if the interface cannot finish initializing. */
'use strict';
const matrixReport = message => window.__TAURI__?.core?.invoke('report_startup', {message}).catch(() => {});
matrixReport('Startup script loaded');
window.matrixStartup = {
  ready: false,
  fail(message) {
    matrixReport(`Interface startup failed: ${message}`);
    const status = document.getElementById('startup-status');
    if (!status) return;
    status.hidden = false;
    status.querySelector('strong').textContent = 'Matrix Play could not finish starting';
    status.querySelector('p').textContent = String(message || 'Please restart Matrix Play. If this continues, report the message shown here.');
    status.querySelector('button').hidden = false;
  },
  complete() {
    this.ready = true;
    matrixReport('Interface ready');
    document.getElementById('startup-status').hidden = true;
  },
};
window.addEventListener('error', event => {
  if (!window.matrixStartup.ready) window.matrixStartup.fail(event.message || 'An interface file could not be loaded.');
});
document.querySelector('#startup-status button').addEventListener('click', () => window.location.reload());
setTimeout(() => {
  if (!window.matrixStartup.ready) window.matrixStartup.fail('The interface is taking longer than expected. Try restarting Matrix Play.');
}, 8000);

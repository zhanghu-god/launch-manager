/* Launch Manager —— 托盘后台启动面板与设置页。 */

const invoke = window.__TAURI__?.core?.invoke;
const listen = window.__TAURI__?.event?.listen;
const $ = (id) => document.getElementById(id);
const systemDark = window.matchMedia('(prefers-color-scheme: dark)');

const state = {
  apps: [],
  selected: 0,
  settings: null,
  view: 'launcher',
};
let toastTimer = null;

function requireInvoke() {
  if (!invoke) throw new Error('未检测到 Tauri 运行环境。');
  return invoke;
}

function errorText(error) {
  return typeof error === 'string' ? error : error?.message || String(error);
}

function showToast(message, type = 'info') {
  clearTimeout(toastTimer);
  const toast = $('toast');
  toast.textContent = message;
  toast.className = `toast show${type === 'error' ? ' error' : ''}`;
  toastTimer = setTimeout(() => { toast.className = 'toast'; }, 2600);
}

function applyTheme() {
  const theme = state.settings?.theme || 'system';
  const dark = theme === 'dark' || (theme === 'system' && systemDark.matches);
  document.documentElement.dataset.theme = dark ? 'dark' : 'light';
}

async function loadSettings() {
  state.settings = await requireInvoke()('get_settings');
  $('scan-dir').value = state.settings.scanDir;
  $('hotkey').value = state.settings.hotkey;
  $('autostart').checked = state.settings.autostart;
  $('hide-after-launch').checked = state.settings.hideAfterLaunch;
  $('theme').value = state.settings.theme;
  applyTheme();
}

async function scanApps() {
  try {
    state.apps = await requireInvoke()('scan_apps');
    state.selected = 0;
    $('scan-dir-label').textContent = state.settings?.scanDir || '';
    renderList();
  } catch (error) {
    state.apps = [];
    renderList();
    showToast(`扫描失败：${errorText(error)}`, 'error');
  }
}

function renderList() {
  const list = $('app-list');
  list.replaceChildren();
  $('empty-state').hidden = state.apps.length > 0;
  state.apps.forEach((app, index) => {
    const item = document.createElement('div');
    item.className = `app-item${index === state.selected ? ' selected' : ''}`;
    item.setAttribute('role', 'option');
    item.setAttribute('aria-selected', String(index === state.selected));

    const badge = document.createElement('span');
    badge.className = 'app-badge';
    badge.textContent = index < 9 ? String(index + 1) : '·';

    const text = document.createElement('div');
    text.className = 'app-text';
    const name = document.createElement('strong');
    name.textContent = app.name;
    const path = document.createElement('span');
    path.textContent = app.exePath;
    text.append(name, path);

    item.append(badge, text);
    item.addEventListener('click', () => launchApp(app));
    item.addEventListener('mouseenter', () => {
      state.selected = index;
      renderList();
    });
    list.appendChild(item);
  });
  const selected = list.children[state.selected];
  if (selected) selected.scrollIntoView({ block: 'nearest' });
}

async function launchApp(app) {
  try {
    await requireInvoke()('launch_app', { exePath: app.exePath });
    if (state.settings?.hideAfterLaunch) await hideWindow();
  } catch (error) {
    showToast(`启动失败：${errorText(error)}`, 'error');
  }
}

async function hideWindow() {
  try {
    await requireInvoke()('hide_window');
  } catch {
    /* 忽略隐藏失败 */
  }
}

function setView(view) {
  state.view = view;
  $('launcher-view').hidden = view !== 'launcher';
  $('settings-view').hidden = view !== 'settings';
  if (view === 'launcher') {
    scanApps();
  } else {
    loadSettings().catch((error) => showToast(`读取设置失败：${errorText(error)}`, 'error'));
  }
}

async function saveSettings(event) {
  event.preventDefault();
  const settings = {
    scanDir: $('scan-dir').value.trim(),
    hotkey: $('hotkey').value.trim(),
    autostart: $('autostart').checked,
    hideAfterLaunch: $('hide-after-launch').checked,
    theme: $('theme').value,
  };
  $('settings-error').hidden = true;
  try {
    state.settings = await requireInvoke()('save_settings', { settings });
    applyTheme();
    showToast('设置已保存');
    scanApps();
  } catch (error) {
    $('settings-error').textContent = errorText(error);
    $('settings-error').hidden = false;
  }
}

function bindEvents() {
  $('open-settings').addEventListener('click', () => setView('settings'));
  $('back-launcher').addEventListener('click', () => setView('launcher'));
  $('settings-form').addEventListener('submit', saveSettings);
  $('rescan-btn').addEventListener('click', async () => {
    await scanApps();
    showToast('已重新扫描');
  });
  $('quit-btn').addEventListener('click', () => requireInvoke()('quit_app'));

  document.addEventListener('keydown', (event) => {
    if (event.key === 'Escape') {
      event.preventDefault();
      hideWindow();
      return;
    }
    if (state.view !== 'launcher') return;
    const tag = event.target.tagName;
    if (tag === 'INPUT' || tag === 'SELECT' || tag === 'TEXTAREA') return;

    if (event.key === 'ArrowDown') {
      event.preventDefault();
      state.selected = Math.min(state.selected + 1, state.apps.length - 1);
      renderList();
    } else if (event.key === 'ArrowUp') {
      event.preventDefault();
      state.selected = Math.max(state.selected - 1, 0);
      renderList();
    } else if (event.key === 'Enter') {
      event.preventDefault();
      if (state.apps[state.selected]) launchApp(state.apps[state.selected]);
    } else if (/^[1-9]$/.test(event.key)) {
      const app = state.apps[Number(event.key) - 1];
      if (app) launchApp(app);
    }
  });

  systemDark.addEventListener('change', () => {
    if (state.settings?.theme === 'system') applyTheme();
  });
}

async function init() {
  bindEvents();
  applyTheme();
  try {
    await loadSettings();
    await scanApps();
  } catch (error) {
    showToast(`初始化失败：${errorText(error)}`, 'error');
  }
  if (listen) {
    listen('show-view', (event) => setView(event.payload || 'launcher'));
  }
}

init();

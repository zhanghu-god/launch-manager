/* Launch Manager —— 托盘后台启动面板与设置页。 */

const invoke = window.__TAURI__?.core?.invoke;
const listen = window.__TAURI__?.event?.listen;
const $ = (id) => document.getElementById(id);
const systemDark = window.matchMedia('(prefers-color-scheme: dark)');

const state = {
  apps: [],
  selected: 0,
  settings: null,
  appInfo: null,
  view: 'launcher',
  updateBusy: false,
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
  $('maximize-on-launch').checked = state.settings.maximizeOnLaunch;
  $('theme').value = state.settings.theme;
  applyTheme();
}

async function loadAppInfo() {
  state.appInfo = await requireInvoke()('get_app_info');
  $('app-version').textContent = `v${state.appInfo.version}`;
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
    item.className = 'app-item';
    item.dataset.index = String(index);
    item.setAttribute('role', 'option');

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
    item.addEventListener('mouseenter', () => setSelected(index));
    list.appendChild(item);
  });
  setSelected(state.selected);
}

// 只更新选中样式，不重建 DOM，避免鼠标点击因元素被替换而丢失。
function setSelected(index) {
  state.selected = Math.max(0, Math.min(index, state.apps.length - 1));
  const list = $('app-list');
  [...list.children].forEach((child, i) => {
    const on = i === state.selected;
    child.classList.toggle('selected', on);
    child.setAttribute('aria-selected', String(on));
  });
  const selected = list.children[state.selected];
  if (selected) selected.scrollIntoView({ block: 'nearest' });
}

async function launchApp(app) {
  try {
    await requireInvoke()('launch_app', {
      exePath: app.exePath,
      maximize: !!state.settings?.maximizeOnLaunch,
    });
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
    loadAppInfo().catch(() => {});
  }
}

async function saveSettings(event) {
  event.preventDefault();
  const settings = {
    scanDir: $('scan-dir').value.trim(),
    hotkey: $('hotkey').value.trim(),
    autostart: $('autostart').checked,
    hideAfterLaunch: $('hide-after-launch').checked,
    maximizeOnLaunch: $('maximize-on-launch').checked,
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

function setUpdateStatus(text, isError = false) {
  const status = $('update-status');
  status.textContent = text;
  status.classList.toggle('error', isError);
}

async function checkForUpdate({ manual = false } = {}) {
  if (state.updateBusy) return;
  state.updateBusy = true;
  $('check-update-btn').disabled = true;
  setUpdateStatus('正在检查更新…');
  try {
    const info = await requireInvoke()('check_update');
    if (!info.hasUpdate) {
      setUpdateStatus(`已是最新版本 v${info.currentVersion}`);
      if (manual) showToast('已是最新版本');
      return;
    }
    setUpdateStatus(`发现新版本 v${info.latestVersion}，正在下载并校验…`);
    const prepared = await requireInvoke()('prepare_update', { info });
    setUpdateStatus(`新版本 v${prepared.version} 已准备好`);
    $('update-version').textContent = `v${prepared.version}`;
    $('update-dialog').showModal();
  } catch (error) {
    setUpdateStatus(`更新失败：${errorText(error)}`, true);
    if (manual) showToast(`更新失败：${errorText(error)}`, 'error');
  } finally {
    state.updateBusy = false;
    $('check-update-btn').disabled = false;
  }
}

async function restartForUpdate() {
  $('update-restart-btn').disabled = true;
  $('update-later-btn').disabled = true;
  try {
    await requireInvoke()('apply_update_and_restart');
  } catch (error) {
    $('update-restart-btn').disabled = false;
    $('update-later-btn').disabled = false;
    showToast(`重启更新失败：${errorText(error)}`, 'error');
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
  $('check-update-btn').addEventListener('click', () => checkForUpdate({ manual: true }));
  $('update-later-btn').addEventListener('click', () => {
    $('update-dialog').close();
    showToast('更新已保留，可稍后重启更新');
  });
  $('update-restart-btn').addEventListener('click', restartForUpdate);

  document.addEventListener('keydown', (event) => {
    if ($('update-dialog').open) return;
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
      setSelected(state.selected + 1);
    } else if (event.key === 'ArrowUp') {
      event.preventDefault();
      setSelected(state.selected - 1);
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
    await loadAppInfo();
    await scanApps();
    checkForUpdate({ manual: false });
  } catch (error) {
    showToast(`初始化失败：${errorText(error)}`, 'error');
  }
  if (listen) {
    listen('show-view', (event) => setView(event.payload || 'launcher'));
  }
}

init();

/* Launch Manager —— 托盘后台启动面板与设置页。 */

const { invoke, listen, $, errorText, systemDark, requireInvoke } = window.PKUI;
const showToast = (message, type) => window.PKUI.toast(message, type);
const dialog = window.__TAURI__?.dialog;

const state = {
  apps: [],
  selected: 0,
  settings: null,
  appInfo: null,
  view: 'launcher',
  updateBusy: false,
};
function applyTheme() {
  window.PKUI.applyTheme(state.settings?.theme || 'system');
}

async function loadSettings() {
  state.settings = await requireInvoke()('get_settings');
  $('scan-dir').value = state.settings.scanDir;
  $('hotkey').value = state.settings.hotkey;
  $('autostart').checked = state.settings.autostart;
  $('hide-after-launch').checked = state.settings.hideAfterLaunch;
  $('maximize-on-launch').checked = state.settings.maximizeOnLaunch;
  $('desktop-icons-double-click').checked = !!state.settings.desktopDoubleClickIcons;
  $('theme').value = state.settings.theme;
  const w = state.settings.webdav || {};
  $('wd-enabled').checked = !!w.enabled;
  $('wd-url').value = w.url || '';
  $('wd-username').value = w.username || '';
  $('wd-password').value = w.password || '';
  $('wd-root').value = w.root || 'rust-apps';
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
    $('empty-scan-dir').textContent = state.settings?.scanDir || '';
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
    renderDataList().catch(() => {});
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
    desktopDoubleClickIcons: $('desktop-icons-double-click').checked,
    theme: $('theme').value,
    webdav: {
      enabled: $('wd-enabled').checked,
      url: $('wd-url').value.trim(),
      username: $('wd-username').value.trim(),
      password: $('wd-password').value,
      root: $('wd-root').value.trim() || 'rust-apps',
    },
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

async function renderDataList() {
  const list = $('data-app-list');
  list.replaceChildren();
  const apps = await requireInvoke()('list_data_apps');
  if (!apps.length) {
    const empty = document.createElement('p');
    empty.className = 'field-hint';
    empty.textContent = '没有扫描到软件，请先在上方配置软件目录。';
    list.appendChild(empty);
    return;
  }
  apps.forEach((app) => {
    const row = document.createElement('div');
    row.className = 'data-row';
    const name = document.createElement('strong');
    name.textContent = app.id === 'self' ? `${app.name}（自身配置）` : app.name;
    const actions = document.createElement('div');
    actions.className = 'header-actions';
    actions.append(
      dataButton('导出', () => exportApp(app)),
      dataButton('导入', () => importApp(app)),
      dataButton('云恢复', () => restoreApp(app)),
    );
    row.append(name, actions);
    list.appendChild(row);
  });
}

function dataButton(label, onClick) {
  const btn = document.createElement('button');
  btn.type = 'button';
  btn.className = 'button secondary';
  btn.textContent = label;
  btn.addEventListener('click', onClick);
  return btn;
}

async function exportApp(app) {
  try {
    const path = await dialog.save({
      defaultPath: `${app.name}-data.json`,
      filters: [{ name: 'JSON 数据', extensions: ['json'] }],
    });
    if (!path) return;
    await requireInvoke()('export_app_data', { appKey: appKey(app), dirPath: app.dirPath, path });
    showToast(`已导出 ${app.name}`);
  } catch (error) {
    showToast(`导出失败：${errorText(error)}`, 'error');
  }
}

async function importApp(app) {
  try {
    const path = await dialog.open({
      multiple: false,
      filters: [{ name: 'JSON 数据', extensions: ['json'] }],
    });
    if (!path) return;
    const report = await requireInvoke()('import_app_data', { appKey: appKey(app), dirPath: app.dirPath, path });
    showToast(`已导入 ${report.written} 个文件，旧数据已备份${app.id === 'self' ? '，重启管理器后生效' : ''}`);
  } catch (error) {
    showToast(`导入失败：${errorText(error)}`, 'error');
  }
}

async function restoreApp(app) {
  try {
    const ok = await dialog.confirm(`从 WebDAV 恢复 ${app.name} 的数据？现有数据会先自动备份。`, { title: '云恢复' });
    if (!ok) return;
    const report = await requireInvoke()('webdav_restore_app', { appKey: appKey(app), dirPath: app.dirPath });
    showToast(`已恢复 ${report.downloaded} 个文件，旧数据已备份${app.id === 'self' ? '，重启管理器后生效' : ''}`);
  } catch (error) {
    showToast(`云恢复失败：${errorText(error)}`, 'error');
  }
}

function setWdStatus(text, isError = false) {
  const el = $('wd-status');
  el.textContent = text;
  el.classList.toggle('error', isError);
}

// 远端同步键 = 主 exe 主名（小写），与后端 exe_key 及各 app 自带同步键一致。
function appKey(app) {
  const base = (app.exePath || '').split(/[\\/]/).pop() || '';
  return base.replace(/\.exe$/i, '').toLowerCase();
}

async function webdavTest() {
  setWdStatus('正在测试连接…');
  try {
    const msg = await requireInvoke()('webdav_test');
    setWdStatus(msg);
  } catch (error) {
    setWdStatus(`连接失败：${errorText(error)}`, true);
  }
}

async function webdavBackup() {
  setWdStatus('正在备份全部软件数据…');
  try {
    const reports = await requireInvoke()('webdav_backup_all');
    const uploaded = reports.reduce((n, r) => n + r.uploaded, 0);
    setWdStatus(`备份完成：${reports.length} 个软件，上传 ${uploaded} 个文件`);
    showToast('云备份完成');
  } catch (error) {
    setWdStatus(`备份失败：${errorText(error)}`, true);
  }
}

function bindEvents() {
  $('open-settings').addEventListener('click', () => setView('settings'));
  $('wd-test-btn').addEventListener('click', webdavTest);
  $('wd-backup-btn').addEventListener('click', webdavBackup);
  $('rescan-launcher').addEventListener('click', async () => {
    await scanApps();
    showToast('已重新扫描');
  });
  $('empty-rescan').addEventListener('click', async () => {
    await scanApps();
    showToast('已重新扫描');
  });
  $('back-launcher').addEventListener('click', () => setView('launcher'));
  $('settings-form').addEventListener('submit', saveSettings);
  $('toggle-desktop-icons').addEventListener('click', async () => {
    try {
      const hidden = await requireInvoke()('toggle_desktop_icons');
      showToast(hidden ? '已隐藏桌面图标' : '已显示桌面图标');
    } catch (error) {
      showToast(`切换桌面图标失败：${errorText(error)}`, 'error');
    }
  });
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

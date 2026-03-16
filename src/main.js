const { invoke } = globalThis.__TAURI__.core;
const { listen } = globalThis.__TAURI__.event;
const { open } = globalThis.__TAURI__.dialog;

const SUPPORTED_EXTENSIONS = ['png', 'jpg', 'jpeg', 'webp', 'avif'];

// ── DOM Elements ─────────────────────────────────────────

const $ = (id) => document.getElementById(id);

const els = {
  header: $('app-header'),
  usageBadge: $('usage-badge'),
  usageCount: $('usage-count'),
  settingsBtn: $('settings-btn'),

  // Setup
  setupView: $('setup-view'),
  setupKeyInput: $('setup-key-input'),
  setupKeyBtn: $('setup-key-btn'),
  setupError: $('setup-error'),

  // Main
  mainView: $('main-view'),
  dropZone: $('drop-zone'),
  resultsContainer: $('results-container'),
  resultsList: $('results-list'),
  resultsTitle: $('results-title'),
  newBatchBtn: $('new-batch-btn'),

  // Settings
  settingsOverlay: $('settings-overlay'),
  settingsCloseBtn: $('settings-close-btn'),
  settingsKeyInput: $('settings-key-input'),
  settingsKeyBtn: $('settings-key-btn'),
  settingsKeyError: $('settings-key-error'),
  outputDirDisplay: $('output-dir-display'),
  outputDirBtn: $('output-dir-btn'),
  outputDirReset: $('output-dir-reset'),
  sendToBtn: $('send-to-btn'),

  // Toast
  toastContainer: $('toast-container'),
};

// ── State ────────────────────────────────────────────────

let state = {
  hasApiKey: false,
  outputDir: null,
  compressionCount: 0,
  isCompressing: false,
  completedCount: 0,
  totalCount: 0,
  totalSaved: 0,
};

// ── Init ─────────────────────────────────────────────────

async function init() {
  try {
    const config = await invoke('get_config');
    state.hasApiKey = config.has_api_key;
    state.outputDir = config.output_dir;
    state.compressionCount = config.compression_count || 0;

    if (state.hasApiKey) {
      showView('main');
    } else {
      showView('setup');
    }

    updateOutputDirDisplay();
    updateUsageBadge();
  } catch (e) {
    console.error('Failed to load config:', e);
    showView('setup');
  }

  // Listen for compression progress
  listen('compress-progress', (event) => {
    handleProgress(event.payload);
  });

  // Listen for files from CLI (Send To)
  listen('files-from-cli', (event) => {
    const files = event.payload;
    if (files && files.length > 0 && state.hasApiKey) {
      startCompression(files);
    }
  });

  // Tauri native drag-and-drop events
  listen('tauri://drag-enter', () => {
    if (!state.isCompressing && state.hasApiKey) {
      els.dropZone.classList.add('drag-over');
    }
  });

  listen('tauri://drag-leave', () => {
    els.dropZone.classList.remove('drag-over');
  });

  listen('tauri://drag-drop', (event) => {
    els.dropZone.classList.remove('drag-over');
    if (state.isCompressing || !state.hasApiKey) return;

    const paths = (event.payload.paths || [])
      .filter((p) => {
        const ext = p.split('.').pop().toLowerCase();
        return SUPPORTED_EXTENSIONS.includes(ext);
      });

    if (paths.length === 0) {
      showToast('No supported files found (PNG, JPEG, WebP, AVIF)', 'error');
      return;
    }

    const totalDropped = (event.payload.paths || []).length;
    if (paths.length < totalDropped) {
      const skipped = totalDropped - paths.length;
      showToast(`${skipped} unsupported file(s) skipped`, 'error');
    }

    startCompression(paths);
  });

  bindEvents();
}

// ── Views ────────────────────────────────────────────────

function showView(view) {
  els.setupView.classList.toggle('active', view === 'setup');
  els.mainView.classList.toggle('active', view === 'main');
  els.header.classList.toggle('hidden', view === 'setup');
}

function showDropZone() {
  els.dropZone.classList.remove('hidden');
  els.resultsContainer.classList.add('hidden');
  state.isCompressing = false;
}

function showResults() {
  els.dropZone.classList.add('hidden');
  els.resultsContainer.classList.remove('hidden');
}

// ── Event Bindings ───────────────────────────────────────

function bindEvents() {
  // Setup
  els.setupKeyBtn.addEventListener('click', handleSetupKey);
  els.setupKeyInput.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') handleSetupKey();
  });

  // Drop zone click to browse
  els.dropZone.addEventListener('click', handleBrowseFiles);

  // New batch
  els.newBatchBtn.addEventListener('click', () => {
    showDropZone();
    els.resultsList.innerHTML = '';
  });

  // Settings
  els.settingsBtn.addEventListener('click', openSettings);
  els.settingsCloseBtn.addEventListener('click', closeSettings);
  els.settingsOverlay.addEventListener('click', (e) => {
    if (e.target === els.settingsOverlay) closeSettings();
  });

  els.settingsKeyBtn.addEventListener('click', handleSettingsKey);
  els.settingsKeyInput.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') handleSettingsKey();
  });

  els.outputDirBtn.addEventListener('click', handleBrowseOutputDir);
  els.outputDirReset.addEventListener('click', handleResetOutputDir);
  els.sendToBtn.addEventListener('click', handleInstallSendTo);
}

// ── Handlers ─────────────────────────────────────────────

async function handleSetupKey() {
  const key = els.setupKeyInput.value.trim();
  if (!key) return;

  els.setupKeyBtn.disabled = true;
  els.setupKeyBtn.textContent = '...';
  els.setupError.classList.add('hidden');

  try {
    const result = await invoke('set_api_key', { key });
    state.hasApiKey = true;
    state.compressionCount = result.compression_count;
    updateUsageBadge();
    showView('main');
    showToast('API key saved successfully', 'success');
  } catch (e) {
    els.setupError.textContent = e;
    els.setupError.classList.remove('hidden');
  } finally {
    els.setupKeyBtn.disabled = false;
    els.setupKeyBtn.textContent = 'Validate';
  }
}

async function handleSettingsKey() {
  const key = els.settingsKeyInput.value.trim();
  if (!key) return;

  els.settingsKeyBtn.disabled = true;
  els.settingsKeyBtn.textContent = '...';
  els.settingsKeyError.classList.add('hidden');

  try {
    const result = await invoke('set_api_key', { key });
    state.hasApiKey = true;
    state.compressionCount = result.compression_count;
    updateUsageBadge();
    els.settingsKeyInput.value = '';
    showToast('API key updated', 'success');
  } catch (e) {
    els.settingsKeyError.textContent = e;
    els.settingsKeyError.classList.remove('hidden');
  } finally {
    els.settingsKeyBtn.disabled = false;
    els.settingsKeyBtn.textContent = 'Save';
  }
}


async function handleBrowseFiles() {
  if (state.isCompressing) return;

  try {
    const selected = await open({
      multiple: true,
      filters: [
        {
          name: 'Images',
          extensions: SUPPORTED_EXTENSIONS,
        },
      ],
    });

    if (selected) {
      const paths = Array.isArray(selected) ? selected : [selected];
      if (paths.length > 0) {
        startCompression(paths);
      }
    }
  } catch (e) {
    console.error('Browse failed:', e);
  }
}

async function handleBrowseOutputDir() {
  try {
    const dir = await open({
      directory: true,
      multiple: false,
    });

    if (dir) {
      await invoke('set_output_dir', { dir });
      state.outputDir = dir;
      updateOutputDirDisplay();
      showToast('Output directory updated', 'success');
    }
  } catch (e) {
    console.error('Browse output dir failed:', e);
  }
}

async function handleResetOutputDir() {
  try {
    await invoke('set_output_dir', { dir: '' });
    state.outputDir = null;
    updateOutputDirDisplay();
    showToast('Output directory reset to default', 'success');
  } catch (e) {
    console.error('Reset output dir failed:', e);
  }
}

async function handleInstallSendTo() {
  els.sendToBtn.disabled = true;
  els.sendToBtn.innerHTML = '<div class="spinner"></div>';

  try {
    await invoke('install_send_to_shortcut');
    els.sendToBtn.textContent = 'Installed ✓';
    showToast('Send To shortcut installed!', 'success');
  } catch (e) {
    showToast('Failed: ' + e, 'error');
    els.sendToBtn.textContent = 'Install';
    els.sendToBtn.disabled = false;
  }
}

// ── Compression ──────────────────────────────────────────

function getFileName(filePath) {
  return filePath.split('\\').pop().split('/').pop();
}

async function startCompression(paths) {
  state.isCompressing = true;
  state.completedCount = 0;
  state.totalCount = paths.length;
  state.totalSaved = 0;

  // Pre-populate the list with all files showing preview + pending status
  els.resultsList.innerHTML = '';
  paths.forEach((filePath, index) => {
    const item = createResultItem(index, getFileName(filePath), 'pending', {}, filePath);
    els.resultsList.appendChild(item);
  });

  els.resultsTitle.textContent = `Compressing... (0/${paths.length})`;
  showResults();

  try {
    await invoke('compress_files', { paths });
  } catch (e) {
    showToast('Compression error: ' + e, 'error');
    state.isCompressing = false;
  }
}

function handleProgress(data) {
  const { file_name, status, original_size, compressed_size, error, compression_count, index, total } = data;

  if (compression_count) {
    state.compressionCount = compression_count;
    updateUsageBadge();
  }

  const item = document.getElementById(`result-${index}`);
  if (!item) return;

  if (status === 'compressing') {
    updateResultItem(item, file_name, 'compressing');
    return;
  }

  if (status === 'done') {
    state.completedCount++;
    const savings = original_size > 0
      ? Math.round((1 - compressed_size / original_size) * 100)
      : 0;
    state.totalSaved += (original_size - compressed_size);
    updateResultItem(item, file_name, 'done', { original_size, compressed_size, savings });
  }

  if (status === 'error') {
    state.completedCount++;
    updateResultItem(item, file_name, 'error', { error });
  }

  if (state.completedCount >= total) {
    const savedStr = formatBytes(state.totalSaved);
    els.resultsTitle.textContent = `Done — saved ${savedStr} total`;
    state.isCompressing = false;
  } else {
    els.resultsTitle.textContent = `Compressing... (${state.completedCount}/${total})`;
  }
}

// ── Result Items ─────────────────────────────────────────

function createResultItem(index, fileName, status, data = {}, filePath = null) {
  const item = document.createElement('div');
  item.className = 'result-item';
  item.id = `result-${index}`;

  // Image thumbnail
  const thumb = document.createElement('div');
  thumb.className = `result-thumb ${status}`;
  if (filePath) {
    const img = document.createElement('img');
    img.alt = fileName;
    thumb.appendChild(img);
    // Load thumbnail asynchronously via Rust
    invoke('read_image_thumbnail', { path: filePath }).then((dataUrl) => {
      img.src = dataUrl;
    }).catch(() => {});
  }
  // Overlay icon for status
  const overlay = document.createElement('div');
  overlay.className = 'result-thumb-overlay';
  overlay.innerHTML = getStatusOverlay(status);
  thumb.appendChild(overlay);

  const info = document.createElement('div');
  info.className = 'result-info';

  const name = document.createElement('div');
  name.className = 'result-name';
  name.textContent = fileName;

  const statusText = document.createElement('div');
  statusText.className = 'result-status';
  statusText.textContent = getStatusText(status, data);

  info.appendChild(name);
  info.appendChild(statusText);

  item.appendChild(thumb);
  item.appendChild(info);

  appendSavingsBadge(item, status, data);

  return item;
}

function updateResultItem(item, fileName, status, data = {}) {
  const thumb = item.querySelector('.result-thumb');
  thumb.className = `result-thumb ${status}`;
  const overlay = thumb.querySelector('.result-thumb-overlay');
  overlay.innerHTML = getStatusOverlay(status);

  const statusText = item.querySelector('.result-status');
  statusText.textContent = getStatusText(status, data);

  // Remove old savings/error badge
  const oldExtra = item.querySelector('.result-savings');
  if (oldExtra) oldExtra.remove();

  appendSavingsBadge(item, status, data);
}

function appendSavingsBadge(item, status, data) {
  if (status === 'done' && data.savings !== undefined) {
    const savings = document.createElement('div');
    savings.className = 'result-savings';
    savings.textContent = `-${data.savings}%`;
    item.appendChild(savings);
  }
  if (status === 'error') {
    const errText = document.createElement('div');
    errText.className = 'result-savings error-text';
    errText.textContent = 'Failed';
    item.appendChild(errText);
  }
}

function getStatusOverlay(status) {
  if (status === 'pending') return '';
  if (status === 'compressing') return '<div class="spinner"></div>';
  if (status === 'done') {
    return '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"/></svg>';
  }
  if (status === 'error') {
    return '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>';
  }
  return '';
}

function getStatusText(status, data) {
  if (status === 'pending') return 'Waiting...';
  if (status === 'compressing') return 'Uploading and compressing...';
  if (status === 'done') {
    return `${formatBytes(data.original_size)} → ${formatBytes(data.compressed_size)}`;
  }
  if (status === 'error') return data.error || 'Unknown error';
  return '';
}

// ── Settings ─────────────────────────────────────────────

function openSettings() {
  els.settingsOverlay.classList.add('active');
  els.settingsKeyError.classList.add('hidden');
  els.settingsKeyInput.value = '';
}

function closeSettings() {
  els.settingsOverlay.classList.remove('active');
}

function updateOutputDirDisplay() {
  if (state.outputDir) {
    els.outputDirDisplay.textContent = state.outputDir;
    els.outputDirDisplay.classList.remove('placeholder');
  } else {
    els.outputDirDisplay.textContent = 'Default (compressed/ subfolder)';
    els.outputDirDisplay.classList.add('placeholder');
  }
}

function updateUsageBadge() {
  if (state.compressionCount > 0) {
    els.usageCount.textContent = state.compressionCount;
    els.usageBadge.classList.remove('hidden');
  }
}

// ── Toast ────────────────────────────────────────────────

function showToast(message, type = 'success') {
  const toast = document.createElement('div');
  toast.className = `toast ${type}`;
  toast.textContent = message;
  els.toastContainer.appendChild(toast);

  setTimeout(() => {
    toast.style.opacity = '0';
    toast.style.transform = 'translateY(12px)';
    toast.style.transition = '0.3s ease-out';
    setTimeout(() => toast.remove(), 300);
  }, 3000);
}

// ── Utilities ────────────────────────────────────────────

function formatBytes(bytes) {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return Number.parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
}

// ── Boot ─────────────────────────────────────────────────

document.addEventListener('DOMContentLoaded', init);

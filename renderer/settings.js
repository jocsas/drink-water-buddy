const form = document.getElementById('settings-form');
const nameInput = document.getElementById('name');
const intervalInput = document.getElementById('interval');
const snoozeInput = document.getElementById('snooze');
const themeSelect = document.getElementById('theme');
const preview = document.getElementById('preview');
const previewTitle = document.getElementById('preview-title');
const previewText = document.getElementById('preview-text');
const cancelBtn = document.getElementById('cancel');

const themes = window.HYDRATE_THEMES;

Object.values(themes).forEach((theme) => {
  const option = document.createElement('option');
  option.value = theme.id;
  option.textContent = theme.label;
  themeSelect.appendChild(option);
});

function clamp(value, min, max, fallback) {
  const parsed = Number.parseInt(value, 10);
  if (!Number.isFinite(parsed)) return fallback;
  return Math.max(min, Math.min(max, parsed));
}

function applyPreview(themeId) {
  const theme = themes[themeId] || themes.default;
  const vars = theme.cssVars || {};
  preview.style.setProperty('--preview-bg', vars['--bubble-bg'] || '#fdfdf7');
  preview.style.setProperty('--preview-border', vars['--bubble-border'] || '#1c1c1c');
  preview.style.setProperty('--preview-text', vars['--bubble-text'] || '#1c1c1c');
  preview.style.setProperty('--preview-shadow', vars['--bubble-shadow'] || 'rgba(0, 0, 0, 0.16)');
  previewTitle.textContent = theme.label;
  previewText.textContent = theme.prompts[0];
}

function fill(settings) {
  nameInput.value = settings.name || '';
  intervalInput.value = settings.intervalMin || 45;
  snoozeInput.value = settings.snoozeMin || 10;
  themeSelect.value = settings.themeId || 'default';
  applyPreview(themeSelect.value);
}

function readForm() {
  return {
    name: nameInput.value,
    intervalMin: clamp(intervalInput.value, 1, 240, 45),
    snoozeMin: clamp(snoozeInput.value, 1, 120, 10),
    themeId: themeSelect.value,
  };
}

async function saveSettings(event) {
  event.preventDefault();
  await window.hydrate.saveSettings(readForm());
  window.hydrate.closeSettingsWindow();
}

window.hydrate.getSettings().then((settings) => {
  fill(settings);
  nameInput.focus();
  nameInput.select();
});

window.hydrate.onSettingsUpdated(fill);

themeSelect.addEventListener('change', () => applyPreview(themeSelect.value));
form.addEventListener('submit', saveSettings);
cancelBtn.addEventListener('click', () => window.hydrate.closeSettingsWindow());
window.addEventListener('keydown', (event) => {
  if (event.key === 'Escape') window.hydrate.closeSettingsWindow();
});

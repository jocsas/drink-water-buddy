const pet = document.getElementById('pet');
const spriteIdle = document.getElementById('sprite-idle');
const spriteDrinking = document.getElementById('sprite-drinking');
const bubble = document.getElementById('bubble');
const bubbleText = document.getElementById('bubble-text');
const buttons = document.getElementById('buttons');
const yesBtn = document.getElementById('yes-btn');
const snoozeBtn = document.getElementById('snooze-btn');
const confetti = document.getElementById('confetti');

let busy = false; // ignore clicks while an animation is running
let currentSettings = {
  name: '',
  intervalMin: 45,
  snoozeMin: 10,
  themeId: 'default',
};
let activeTheme = window.HYDRATE_THEMES.default;

const glyphs = {
  drop: '💧',
  sparkle: '✨',
  heart: '💙',
  bubble: '🫧',
  star: '⭐',
  diamond: '◆',
  dot: '•',
  ship: '▲',
};

const pick = (arr) => arr[(Math.random() * arr.length) | 0];

function themeFor(themeId) {
  return window.HYDRATE_THEMES[themeId] || window.HYDRATE_THEMES.default;
}

function format(text, values = {}) {
  return String(text || '').replace(/\{(\w+)\}/g, (_match, key) => values[key] ?? '');
}

function applyTheme(themeId) {
  activeTheme = themeFor(themeId);
  document.body.dataset.theme = activeTheme.id;

  Object.entries(activeTheme.cssVars || {}).forEach(([property, value]) => {
    document.getElementById('stage').style.setProperty(property, value);
  });

  yesBtn.textContent = activeTheme.buttons.yes;
  snoozeBtn.textContent = activeTheme.buttons.snooze;
  updateSprites();
  showDrinking(false);
}

function applySettings(nextSettings = {}) {
  currentSettings = {
    ...currentSettings,
    ...nextSettings,
    name: String(nextSettings.name ?? currentSettings.name ?? '').trim(),
  };
  applyTheme(currentSettings.themeId);
}

function promptFor() {
  const name = currentSettings.name;
  const lines = name ? activeTheme.namedPrompts : activeTheme.prompts;
  return format(pick(lines), { name, minutes: currentSettings.snoozeMin });
}

function cheerFor() {
  const name = currentSettings.name;
  const lines = name ? activeTheme.namedCheers : activeTheme.cheers;
  return format(pick(lines), { name, minutes: currentSettings.snoozeMin });
}

function wait(ms) {
  return new Promise((r) => setTimeout(r, ms));
}

function themeSpritePath(theme, state) {
  return `../${theme.assetFolder}/${state}.png`;
}

function uniqueSources(sources) {
  return sources.filter((src, index) => src && sources.indexOf(src) === index);
}

function setSprite(img, sources) {
  const candidates = uniqueSources(sources);
  if (img.dataset.currentSrc === candidates[0]) return;

  let index = 0;
  img.onerror = () => {
    index += 1;
    if (!candidates[index]) return;
    img.dataset.currentSrc = candidates[index];
    img.src = candidates[index];
  };
  img.dataset.currentSrc = candidates[0];
  img.src = candidates[0];
}

function updateSprites() {
  const defaultTheme = window.HYDRATE_THEMES.default;
  const idleFallback = themeSpritePath(defaultTheme, 'idle');
  const drinkingFallback = themeSpritePath(defaultTheme, 'drinking');
  const idlePath = themeSpritePath(activeTheme, 'idle');
  const drinkingPath = themeSpritePath(activeTheme, 'drinking');

  setSprite(spriteIdle, [idlePath, idleFallback]);
  setSprite(spriteDrinking, [drinkingPath, idlePath, drinkingFallback, idleFallback]);
}

function showDrinking(on) {
  spriteIdle.classList.toggle('hidden', on);
  spriteDrinking.classList.toggle('hidden', !on);
}

// ------------------------------------------------------------ walk in / out
async function walkIn() {
  showDrinking(false);
  pet.classList.remove('celebrate');
  pet.classList.remove('face-right'); // face forward as she arrives
  pet.classList.add('offstage');
  // force reflow so the transition runs from the offstage position
  void pet.offsetWidth;
  pet.classList.add('walking');
  pet.classList.remove('offstage');
  await wait(1150);
  pet.classList.remove('walking');
}

async function walkOut() {
  bubble.classList.add('hidden');
  pet.classList.add('face-right'); // turn to face the exit direction
  pet.classList.add('walking');
  pet.classList.add('offstage');
  await wait(1150);
  pet.classList.remove('walking');
  pet.classList.remove('face-right');
}

function showBubble(text, withButtons) {
  bubbleText.textContent = text;
  buttons.classList.toggle('hidden', !withButtons);
  bubble.classList.remove('hidden');
}

// -------------------------------------------------------------- celebration
function burstConfetti() {
  const themeGlyphs = activeTheme.confettiGlyphs || window.HYDRATE_THEMES.default.confettiGlyphs;
  const colors = activeTheme.confettiColors || window.HYDRATE_THEMES.default.confettiColors;
  const count = 42;
  for (let i = 0; i < count; i++) {
    const piece = document.createElement('div');
    piece.className = 'confetti-piece';
    const useGlyph = Math.random() < 0.5;
    if (useGlyph) {
      const key = themeGlyphs[(Math.random() * themeGlyphs.length) | 0];
      piece.textContent = glyphs[key] || key;
    } else {
      piece.textContent = '■';
      piece.style.color = colors[(Math.random() * colors.length) | 0];
    }
    piece.style.left = Math.random() * 100 + '%';
    piece.style.setProperty('--dur', 0.9 + Math.random() * 0.9 + 's');
    piece.style.setProperty('--fall', 280 + Math.random() * 160 + 'px');
    piece.style.setProperty('--spin', (Math.random() * 720 - 360) + 'deg');
    piece.style.animationDelay = Math.random() * 0.25 + 's';
    confetti.appendChild(piece);
    setTimeout(() => piece.remove(), 2200);
  }
}

async function celebrate() {
  showBubble(cheerFor(), false);
  showDrinking(true); // she takes a sip
  await wait(950);
  showDrinking(false);
  pet.classList.add('celebrate'); // little happy hop
  burstConfetti();
  await wait(1000);
  pet.classList.remove('celebrate');
}

// ------------------------------------------------------------------- flow
async function runReminder(nextSettings) {
  if (busy) return;
  busy = true;
  applySettings(nextSettings || {});
  await walkIn();
  showBubble(promptFor(), true);
  busy = false;
}

async function onYes() {
  if (busy) return;
  busy = true;
  window.hydrate.yes(); // schedule the next nudge using the configured interval
  await celebrate();
  showBubble(activeTheme.goodbye, false);
  await wait(600);
  await walkOut();
  window.hydrate.hide();
  busy = false;
}

async function onSnooze() {
  if (busy) return;
  busy = true;
  window.hydrate.snooze(); // come back in 10 min
  const template = currentSettings.name ? activeTheme.namedSnooze : activeTheme.snooze;
  showBubble(
    format(template, { name: currentSettings.name, minutes: currentSettings.snoozeMin }),
    false
  );
  await wait(1400);
  await walkOut();
  window.hydrate.hide();
  busy = false;
}

yesBtn.addEventListener('click', onYes);
snoozeBtn.addEventListener('click', onSnooze);

// Triggered by the main process using the configured reminder interval.
window.hydrate.getSettings().then(applySettings);
window.hydrate.onSettingsUpdated(applySettings);
window.hydrate.onShow((payload) => runReminder(payload));

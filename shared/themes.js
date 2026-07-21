(function exposeThemes(root) {
  const themes = {
    default: {
      id: 'default',
      label: 'Default doll',
      trayDrinkLabel: 'Drink now',
      character: 'doll',
      buttons: {
        yes: 'YES, I DRANK',
        snooze: 'SNOOZE',
      },
      cssVars: {
        '--bubble-bg': '#fdfdf7',
        '--bubble-border': '#1c1c1c',
        '--bubble-text': '#1c1c1c',
        '--bubble-shadow': 'rgba(0, 0, 0, 0.18)',
        '--button-yes-bg': '#37c24a',
        '--button-yes-hover': '#2fae41',
        '--button-yes-text': '#ffffff',
        '--button-snooze-bg': '#fdfdf7',
        '--button-snooze-hover': '#ececec',
        '--button-snooze-text': '#1c1c1c',
        '--pet-shadow': 'rgba(0, 0, 0, 0.28)',
      },
      prompts: [
        'Time to drink water to keep your skin glowing!',
        'Hydration check! Take a sip of water',
        'Water break! Your body will thank you.',
        'Psst... a few sips of water? Stay glowing!',
        "Don't forget me - drink some water!",
      ],
      namedPrompts: [
        '{name}, time to drink water to keep your skin glowing!',
        '{name}, hydration check! Take a sip',
        'Water break, {name}! Your body will thank you.',
        'Psst {name}... a few sips of water? Stay glowing!',
        "Don't forget me, {name} - drink some water!",
      ],
      cheers: [
        'Yay! Stay glowing',
        'Amazing! Keep it up',
        "That's the spirit!",
        'Hydrated and happy!',
      ],
      namedCheers: [
        'Yay {name}! Stay glowing',
        'Amazing, {name}! Keep it up',
        "That's the spirit, {name}!",
        'Hydrated and happy, {name}!',
      ],
      snooze: "I'll come back in {minutes} mins!",
      namedSnooze: "I'll come back in {minutes} mins, {name}!",
      goodbye: 'See you in a bit!',
      confettiGlyphs: ['drop', 'sparkle', 'heart', 'bubble', 'star'],
      confettiColors: ['#37c24a', '#4aa3df', '#ffd447', '#ff7a59', '#8ad6ff'],
    },
    starwars: {
      id: 'starwars',
      label: 'Star Wars-inspired',
      trayDrinkLabel: 'Hydration mission',
      character: 'droid',
      buttons: {
        yes: 'SIP COMPLETE',
        snooze: 'SNOOZE JUMP',
      },
      cssVars: {
        '--bubble-bg': '#111827',
        '--bubble-border': '#f8d45c',
        '--bubble-text': '#e7f6ff',
        '--bubble-shadow': 'rgba(0, 229, 255, 0.24)',
        '--button-yes-bg': '#21d07a',
        '--button-yes-hover': '#15b968',
        '--button-yes-text': '#06140d',
        '--button-snooze-bg': '#1d2a3d',
        '--button-snooze-hover': '#263c5c',
        '--button-snooze-text': '#e7f6ff',
        '--pet-shadow': 'rgba(0, 229, 255, 0.36)',
      },
      prompts: [
        'Hydration alert from the outer rim: take a sip.',
        'Canteen check before the next lightspeed jump.',
        'The galaxy runs better when you drink water.',
        'Pilot check: water level low. Sip now.',
        'Tiny droid report: hydration systems need support.',
      ],
      namedPrompts: [
        '{name}, hydration alert from the outer rim.',
        '{name}, canteen check before the next lightspeed jump.',
        'The galaxy needs you hydrated, {name}.',
        'Pilot check, {name}: water level low.',
        'Tiny droid report for {name}: sip protocol ready.',
      ],
      cheers: [
        'Mission complete. Hydration restored.',
        'Systems green. Excellent sip.',
        'Canteen status: victorious.',
        'Good work. Back to patrol.',
      ],
      namedCheers: [
        'Mission complete, {name}. Hydration restored.',
        'Systems green, {name}. Excellent sip.',
        'Canteen status: victorious, {name}.',
        'Good work, {name}. Back to patrol.',
      ],
      snooze: 'Snooze jump set for {minutes} mins.',
      namedSnooze: 'Snooze jump set for {minutes} mins, {name}.',
      goodbye: 'Back to patrol.',
      confettiGlyphs: ['star', 'sparkle', 'diamond', 'dot', 'ship'],
      confettiColors: ['#f8d45c', '#21d4fd', '#21d07a', '#f87171', '#ffffff'],
    },
  };

  if (typeof module === 'object' && module.exports) {
    module.exports = themes;
  } else {
    root.HYDRATE_THEMES = themes;
  }
})(typeof globalThis === 'object' ? globalThis : window);

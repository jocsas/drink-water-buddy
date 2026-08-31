// WebKitGTK can't sync per-pixel-transparent windows on X11 reliably
// (ghost trails); Linux renders an opaque pixel-art card instead.
// Must load before style.css so the class applies before first paint.
if (/Linux/.test(navigator.platform)) document.documentElement.classList.add('card');

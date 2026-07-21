#!/usr/bin/env python3
from pathlib import Path
import sys

from PIL import Image


CANVAS_SIZE = (260, 330)
TRAY_SIZE = 64

FRAME_BOXES = {
    "walk-1": (320, 30, 710, 530),
    "walk-2": (910, 30, 1305, 530),
    "walk-3": (1535, 30, 1925, 530),
    "walk-4": (2130, 30, 2525, 530),
    "idle": (320, 550, 720, 1035),
    "drink-1": (870, 550, 1305, 1035),
    "drink-2": (1495, 550, 1935, 1035),
    "happy": (2120, 550, 2535, 1045),
    "cast": (585, 1035, 1015, 1520),
}

ROOT_EXPORTS = {
    "idle.png": "idle",
    "drinking.png": "drink-2",
}


def is_checkerboard_pixel(pixel):
    r, g, b, alpha = pixel
    if alpha == 0:
        return True
    average = (r + g + b) / 3
    return max(r, g, b) - min(r, g, b) <= 14 and 120 <= average <= 218


def remove_checkerboard(image):
    result = image.convert("RGBA")
    pixels = result.load()
    for y in range(result.height):
        for x in range(result.width):
            if is_checkerboard_pixel(pixels[x, y]):
                pixels[x, y] = (0, 0, 0, 0)
    return result


def alpha_bbox(image):
    alpha = image.getchannel("A")
    return alpha.getbbox()


def normalize_frame(image):
    bbox = alpha_bbox(image)
    if not bbox:
        return Image.new("RGBA", CANVAS_SIZE, (0, 0, 0, 0))

    content = image.crop(bbox)
    max_width = CANVAS_SIZE[0] - 12
    max_height = CANVAS_SIZE[1] - 12
    scale = min(max_width / content.width, max_height / content.height, 1)
    if scale < 1:
        next_size = (max(1, round(content.width * scale)), max(1, round(content.height * scale)))
        content = content.resize(next_size, Image.Resampling.NEAREST)

    canvas = Image.new("RGBA", CANVAS_SIZE, (0, 0, 0, 0))
    x = (CANVAS_SIZE[0] - content.width) // 2
    y = CANVAS_SIZE[1] - content.height - 4
    canvas.alpha_composite(content, (x, y))
    return canvas


def make_tray_icon(frame):
    bbox = alpha_bbox(frame)
    if not bbox:
        return Image.new("RGBA", (TRAY_SIZE, TRAY_SIZE), (0, 0, 0, 0))

    content = frame.crop(bbox)
    scale = min((TRAY_SIZE - 6) / content.width, (TRAY_SIZE - 6) / content.height)
    size = (max(1, round(content.width * scale)), max(1, round(content.height * scale)))
    content = content.resize(size, Image.Resampling.NEAREST)

    icon = Image.new("RGBA", (TRAY_SIZE, TRAY_SIZE), (0, 0, 0, 0))
    icon.alpha_composite(content, ((TRAY_SIZE - size[0]) // 2, (TRAY_SIZE - size[1]) // 2))
    return icon


def main():
    if len(sys.argv) != 2:
        raise SystemExit("Usage: scripts/extract-wizard-theme.py /path/to/wizard-sprite-sheet.png")

    source_path = Path(sys.argv[1]).expanduser()
    out_dir = Path("assets/themes/wizard")
    frames_dir = out_dir / "frames"
    frames_dir.mkdir(parents=True, exist_ok=True)

    sheet = Image.open(source_path).convert("RGBA")
    rendered = {}

    for name, box in FRAME_BOXES.items():
        frame = normalize_frame(remove_checkerboard(sheet.crop(box)))
        rendered[name] = frame
        frame.save(frames_dir / f"{name}.png")

    for output_name, frame_name in ROOT_EXPORTS.items():
        rendered[frame_name].save(out_dir / output_name)

    make_tray_icon(rendered["idle"]).save(out_dir / "tray.png")

    readme = out_dir / "README.md"
    readme.write_text(
        "Wizard theme assets are generated from a sprite sheet.\n\n"
        "Run this from the repo root to regenerate them:\n\n"
        "```bash\n"
        "python3 scripts/extract-wizard-theme.py /path/to/wizard-sprite-sheet.png\n"
        "```\n\n"
        "Root files used by fallback themes:\n\n"
        "- `idle.png`\n"
        "- `drinking.png`\n"
        "- `tray.png`\n\n"
        "Animation frames live in `frames/`.\n",
        encoding="utf-8",
    )


if __name__ == "__main__":
    main()

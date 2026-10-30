"""Generate the Kiln app icon: dark square, hairline inset, Inter K mark.

Run from the repo root:  python3 tools/gen_icon.py
Output: src/app-icon.png (1024x1024), then `npm run icons` derives the set.
"""

from PIL import Image, ImageDraw, ImageFont

SIZE = 1024
BG = (20, 20, 20)          # --bg dark
BORDER = (46, 46, 46)      # --border
TEXT = (232, 232, 232)     # --text primary
FONT = "src/fonts/Inter-SemiBold.ttf"


def main() -> None:
    image = Image.new("RGBA", (SIZE, SIZE), BG + (255,))
    draw = ImageDraw.Draw(image)

    # Hairline inset frame, 1px at 32px scale => 4px at 1024.
    inset = 8
    draw.rectangle(
        [inset, inset, SIZE - inset - 1, SIZE - inset - 1],
        outline=BORDER,
        width=4,
    )

    # Kiln mark: the letter K in the UI's semibold face.
    font = ImageFont.truetype(FONT, 560)
    left, top, right, bottom = draw.textbbox((0, 0), "K", font=font)
    width = right - left
    height = bottom - top
    draw.text(((SIZE - width) / 2 - left, (SIZE - height) / 2 - top), "K",
              font=font, fill=TEXT)

    image.save("src/app-icon.png")
    print("wrote src/app-icon.png")


if __name__ == "__main__":
    main()

from PIL import Image, ImageDraw

SIZE = 512

im = Image.open("public/brand/tidaliq.png").convert("RGB")

cx, cy, half = 634, 628, 370
icon = (
    im.crop((cx - half, cy - half, cx + half, cy + half))
    .resize((SIZE, SIZE), Image.LANCZOS)
    .convert("RGBA")
)
mask = Image.new("L", (SIZE, SIZE), 0)
ImageDraw.Draw(mask).rounded_rectangle((0, 0, SIZE - 1, SIZE - 1), radius=100, fill=255)
icon.putalpha(mask)

icon.save("src/app/icon.png")
print("saved src/app/icon.png")
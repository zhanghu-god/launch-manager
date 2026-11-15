"""重新生成 Tauri 图标源文件；需要 Pillow。"""

from pathlib import Path

from PIL import Image, ImageDraw

out = Path(__file__).with_name("icon.png")
image = Image.new("RGBA", (1024, 1024))
draw = ImageDraw.Draw(image)
draw.rounded_rectangle((64, 64, 960, 960), 220, fill="#2f6fed")
draw.rounded_rectangle((236, 236, 468, 468), 52, fill="white")
draw.rounded_rectangle((556, 236, 788, 468), 52, fill=(255, 255, 255, 217))
draw.rounded_rectangle((236, 556, 468, 788), 52, fill=(255, 255, 255, 217))
draw.line((556, 672, 788, 672), fill="white", width=56)
draw.line((672, 556, 672, 788), fill="white", width=56)
image.save(out)
print(out)

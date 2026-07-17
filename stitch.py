import os
from PIL import Image

textures = [
    "grass_side.png",
    "grass_top.png",
    "dirt.png",
    "stone.png",
    "sand.png",
    "clay.png",
    "gravel.png",
    "oak_log_inside.png",
    "oak_log_outside.png",
    "oak_leaves.png",
    "water.png"
]

images = []
for tex in textures:
    img = Image.open(f"assets/textures/{tex}").convert("RGBA")
    images.append(img)

width = images[0].width
height = images[0].height
total_height = height * len(images)

atlas = Image.new("RGBA", (width, total_height))

for i, img in enumerate(images):
    atlas.paste(img, (0, i * height))

atlas.save("assets/textures/atlas.png")
print("Atlas saved to assets/textures/atlas.png")

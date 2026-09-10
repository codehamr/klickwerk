# Application icon

A light pointer and two mint click rays represent precise desktop interaction.
The solid slate-navy background keeps the mark visible on light and dark
desktop backgrounds.

- `klickwerk.png`: full-resolution design source.
- `klickwerk.ico`: Windows icon with 16, 20, 24, 32, 40, 48, 64, 96, 128,
  and 256-pixel images.

## Build integration

Tauri embeds `assets/klickwerk.ico` in the Windows executable. The existing
PNG and ICO are preserved. The PNG is the editable design source; it is not
loaded at runtime. Normal builds do not regenerate image assets.

## Exporting a replacement

ImageMagick 7 is needed only when intentionally regenerating the ICO after
changing the PNG. From the project root:

```sh
magick assets/klickwerk.png -background none -alpha on -filter Lanczos \
  -define icon:auto-resize=256,128,96,64,48,40,32,24,20,16 \
  -depth 8 assets/klickwerk.ico
make
```

Keep both images in the project. Normal builds need no image tools or network
access.

## Design provenance

Created using the built-in Imagegen tool, then exported to ICO with
ImageMagick. The full-resolution PNG preserves the generated image.

Initial generation prompt:

```text
Use case: logo-brand
Asset type: final square Windows desktop application icon for "klickwerk", a native desktop automation tool.
Primary request: Design a beautiful, exceptionally clean, confident icon that communicates precise mouse clicks and helpful automation, readable at 16 pixels.
Subject: One bold off-white mouse pointer, angled upper-left, geometrically simple with a short stem and a clear notch, plus exactly two small vivid mint/teal click rays near its upper-left tip. The mark should feel expertly balanced, distinctive and quietly premium.
Style/medium: Crisp minimalist vector-like graphic, flat colors, precise edges. A dark midnight-navy rounded-square tile with a barely perceptible lighter upper edge, smooth generous corner radius. No 3D, no bevel, no texture, no complex gradients.
Composition/framing: Square 1024 by 1024 canvas. The tile occupies about 92 percent of the canvas, centered with an even narrow transparent margin. The white cursor is large, centered optically, occupies about 60 percent of the tile. The two mint click rays are thick enough to survive reduction. Strong negative space, comfortable padding; the cursor and rays must not touch the tile edge.
Color palette: midnight navy tile (#142532), warm near-white cursor (#F5F8F7), fresh mint click rays (#58E0B4).
Background: actual alpha transparency outside the rounded-square tile; no painted checkerboard and no outside drop shadow.
Constraints: Render only one finished standalone app icon, no wordmark, no letters, no text, no watermark, no borders around the canvas, no mockup, no presentation sheet, no surrounding decoration. Keep the silhouette simple and recognizable at tiny Windows Explorer sizes.
```

Tile refinement prompt:

```text
Edit target: the supplied klickwerk application icon.
Keep the exact pointer silhouette, placement, size, two mint click rays, rounded-square silhouette and transparent outer margin.
Change only the tile finish and opacity: replace ALL of the tile interior with a single uniform solid medium-dark slate navy #234252 at FULL OPACITY (alpha 255). The input has accidental transparent black patches inside the tile; eliminate every one of these holes. The entire rounded-square tile must be opaque, including all negative space around the cursor. Keep alpha transparency ONLY outside the rounded-square silhouette.
Remove every gradient, highlight, bevel, texture, shadow and metallic effect. The result is an extremely clean flat graphic: fully opaque navy tile, fully opaque near-white cursor, fully opaque mint rays. The tile must look like one continuous solid navy surface on both white and black desktop backgrounds.
No text, no extra elements, no checkerboard. Return just the final square icon image.
```


Final export design prompt:

```text
Edit the supplied app icon. Keep the white pointer and the two mint click rays exactly as they are.
Extend the solid dark slate-navy color of the tile to cover the ENTIRE SQUARE CANVAS, edge to edge, including all four corners. The final image is a completely opaque square app icon. Replace every gray and white checkerboard pixel with the same solid dark navy color. Remove the rounded outer corners and the margin: the navy fills the whole image as a flat, uniform color.
No transparency. No checkerboard. No border. No gradients. No shadow. No bevel. No text. The whole square background is solid navy, and only the pointer and two mint click rays sit on that solid navy field.
Output the finished flat icon as a fully opaque RGB PNG.
```

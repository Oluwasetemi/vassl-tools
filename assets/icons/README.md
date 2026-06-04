# App Icons

Place the following PNG files here before running `cargo bundle`:

| File                | Size       | Used for                      |
|---------------------|------------|-------------------------------|
| `vassl_512.png`     | 512×512 px | macOS Retina .app icon        |
| `vassl_256.png`     | 256×256 px | macOS standard .app icon      |
| `vassl_128.png`     | 128×128 px | macOS small icon              |
| `vassl_32.png`      | 32×32 px   | macOS Dock / Windows taskbar  |
| `vassl.ico`         | multi-size | Windows .exe icon (optional)  |

`cargo-bundle` converts the PNGs to `.icns` automatically on macOS.

To generate all sizes from a single 1024×1024 source using sips (macOS):

```bash
SRC=vassl_1024.png
for size in 512 256 128 32; do
  sips -z $size $size $SRC --out vassl_${size}.png
done
```

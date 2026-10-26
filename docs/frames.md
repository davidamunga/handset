# Device frames

Handset draws the phone inside a frame: a PNG with a transparent hole where the screen goes, plus a one-line
`.frame` file that says where the hole is.

## Built-in frames

Pick one in the right-click menu (Frame), with the Next Frame button on the toolbar, or with
`--frame <name>`. Names are `<device>-<colour>`; the first three are the originals.

| Device | Looks like | Colours |
|---|---|---|
| (classic) | a punch-hole phone | `obsidian`, `porcelain`, `hazel` |
| `notch` | a phone with a notch | `graphite`, `silver` |
| `island` | a pill-shaped camera cutout | `titanium`, `black` |
| `drop` | a waterdrop notch and a chin | `midnight`, `coral` |
| `home` | an older phone with bezels and a home button | `black`, `white` |
| `flat` | flat edges and a punch hole | `slate`, `sage` |
| `tablet` | a 5:8 tablet | `space`, `silver` |
| `fold` | an unfolded foldable, with a crease | `black`, `cream` |
| `flip` | an unfolded flip phone, with a hinge | `lilac`, `black` |
| `rugged` | a rugged phone with a rubber bumper | `orange`, `olive` |

For example `handset --frame tablet-silver`. The shapes are generic device types, not copies of one maker's
product (see ADR 0021). The phone's picture is fitted inside the screen area, so a frame does not have to
match the phone's exact proportions.

## Your own frame

1. Draw a PNG, 8-bit RGBA, with the screen area fully transparent. Leave some transparent margin if you want a
   shadow or buttons that stick out. Aim for 1300 to 2800 pixels tall; larger images are scaled down.
2. Next to it, write a file with the same name and a `.frame` extension holding four numbers, the position
   and size of the transparent screen area in pixels of the PNG: `x y width height`.

   ```
   112 112 1080 2400
   ```

3. Run `handset --frame path/to/my-phone.png`.

Pixels that are opaque in the PNG move the window when dragged, and anything under the transparent hole shows
the phone. A faint overlay inside the hole (for a fold crease, say) is fine as long as it is mostly
transparent: clicks over it still reach the phone.

## Adding a built-in frame

Frames are generated, not drawn by hand:

```sh
python3 frames/make_frames.py app/assets              # regenerate everything (needs Pillow)
python3 frames/make_frames.py /tmp/out --sheet        # also writes sheet.png to compare them all
python3 frames/make_frames.py /tmp/out tablet         # only one device type
```

Add a `Device` (and its colourways) to `frames/make_frames.py`, regenerate, then add the names to the `builtin!`
list in `app/src/frame.rs`. `cargo test` loads every built-in frame and checks that its screen area is inside
the image and see-through.

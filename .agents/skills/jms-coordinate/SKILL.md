---
name: jms-coordinate
description: Reference for JMS's two coordinate systems — HexPos game-grid coordinates and Bevy world-space pixels — covering the pixel_pos mapping, screen orientation, z-layer semantics, and render ordering rules. Use when writing or reviewing code that converts grid positions to transforms, places entities on the hex map, or reasons about screen direction and layer/depth ordering.
---

# JMS Coordinate Systems

JMS has exactly **two** coordinate systems. There is deliberately no intermediate
"map pixel" frame: earlier code defined a y-down pixel frame that `tiling.rs`
then negated into world space. That frame was removed on purpose; do not
reintroduce it or a `center.y - px.y` style flip.

Authoritative sources: `src/components/tile.rs` (constants + mapping),
`src/plugins/tiling.rs` (transform + render ordering, Tiled tile → grid),
`src/third_party/tiled.rs` (Tiled map loading),
`doc/vault/jms/Tech Design/Tiling.md` (design authority; wiki format).

## 1. HexPos — game/grid coordinates

`(x, y, z)` where x/y are quarter-hex units (dx, dy) on the horizontal slice and
z is the hex grid layer index. Hexagon centers and vertices sit on even y.

Key constants (all in `src/components/tile.rs`):

- `DX_PX` = 8 px per dx, `DY_PX` = 5 px per dy (`HEX_WIDTH`/4, `HEX_HEIGHT`/4)
- Neighbor steps: `COLUMN_DX` = 3, `COLUMN_DY` = 2, `ROW_DY` = 4
- `LAYER_STEP_Z` = 11 px vertical distance between layers (doc calls it S)

`HexPos::from_grid(col, row)` staggers odd columns by `+COLUMN_DY`, which renders
as those columns sitting **above** the even-column row line (grid +y is
screen-up; see below).

## 2. Bevy world space — pixels

`HexPos::pixel_pos()` is the **only** grid-to-pixel mapping and returns
world-space pixels directly (a `Vec2` usable in a `Transform`):

```rust
Vec2::new(DX_PX * x, DY_PX * y + LAYER_STEP_Z * z)
```

Screen conventions (deliberate choices, don't "fix" them):

- +x renders right, **+y renders upward** — same as `Transform`. A map's origin
  tile `(0,0)` sits at the bottom-left of the extent.
- Higher z rises: `z=1` is `LAYER_STEP_Z` above `z=0` and draws in front of it;
  `z=-1` sinks and draws behind. This matches Tiling.md's dimetric projection
  ("further on the y axis … display as 'higher' on the screen").
- Consumers must NOT negate y when using `pixel_pos` output.

`tiling.rs` composes the transform as:

```rust
Transform::from_xyz(
    px.x,
    px.y + atlas.sprite_y_offset,
    get_coord_depth(game_pos),
)
```

`sprite_y_offset` is applied in world space, where negative = down: the
square tile cell hangs a foundation strip below the 20 px hexagon. It is
`-(cell_height - HEX_HEIGHT_PX) / 2`, per tileset (-6 for the 32x32 cells of
`Tiling.md`).

## 3. Tiled tile coordinates — map files

Tiles come from Tiled maps (`assets/tiled/*.tmx`, loaded by
`src/third_party/tiled.rs`). A Tiled tile `(x, y)` is only a map-file index;
`tiling.rs`'s `tile_grid_pos` converts it to a `GridPos` before anything else:

- Tiled's y points **down**, so rows are flipped: Tiled's first row is the
  top of the map on screen (the highest world y).
- Both axes are shifted to center the map on the world origin. The **column
  shift must be even**: hex centers only exist where `col + drow` is even, so
  an odd shift would put every tile off the lattice.
- Tiled has no elevation: a tile layer's `offsety` stands in for the z layer,
  in whole steps of `LAYER_STEP_Z` (`-11` px = z `+1`, since negative is up
  on screen). Layers with other offsets are skipped with a warning.
- Maps are "Hexagonal (Staggered)", stagger axis x, index odd, finite; other
  layouts fail to load.

## Render ordering rules

- Layers are separated in render-z by `LAYER_ORDER_STEP` (1.0), which must stay
  larger than one layer's whole y-order span — enforced by the compile-time
  `TILE_ORDER_Y_RANGE < LAYER_ORDER_STEP` assert, so layers never interleave.
- Within a layer, tiles lower on the screen draw over tiles above them
  (Tiling.md: a tile's top 10 px must cover the bottom 22 px of the tile above).
  Grid +y is screen-up, so screen-lower means smaller y — hence the negated
  depth term `- pos.y * TILE_ORDER_Y_SCALE`.

## Pitfalls

- No sign flips when consuming `pixel_pos`; it already emits world space.
- Don't flip z's sign in `pixel_pos`: layers would sink while still drawing in
  front, disagreeing with the elevation illusion.
- Grid +y is the opposite of image/atlas row order (atlas indices still traverse
  top-left, row by row downward). Grid direction and image direction are
  unrelated.
- Test comments in `tile.rs` describe directions in screen terms ("above the row
  line"); keep them consistent if you touch the stagger.

If conventions ever change, update together: `pixel_pos` and its doc comment,
the transform/depth term in `tiling.rs`, the `tile.rs` tests, and reflect the
decision in `Tiling.md` (its Projection section currently leaves the y screen
direction unspecified; the code interprets it as +y up).

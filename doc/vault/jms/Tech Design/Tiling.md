[[JMS]] is a tile-based game: [[entities]] are sometimes aligned relative to tiles in a grid, at specific coordinates. In this case, the tiles are hexagons.

For this document, sqrt3 means the square root of 3.
## Properties

- Flat top: with "QWEASD" keyboard controls, we can move north-west, north, north-east, south-west, south, south-east
- Hexes centers are separated by an arbitrary center-to-center distance D. This makes each hexagon D tall and 2D/sqrt3 wide
- One hex grid is 2d, but the world exists in 3d, so one hex grid exists on each z level
- Coordinates are in (x,y,z), where z represents the hex grid layer, and (x,y) represents the horizontal and vertical position on that grid. (x,y) are in quarter-hex width and quarter-hex height.
	- When talking about a single grid, "horizontal" and "vertical" correspond to the x and y axis of this horizontal slice
	- When talking about the game space generally, the axes are the lateral axis (x), longitudinal axis (y), and the vertical axis (z). Horizontal means along the lateral/longitudinal plane.

## Formulas

- D: distance to neighbor (arbitrary unit of distance, but roughly as wide as a person)
- H: height of the whole hexagon (flat-to-flat) = D
- W: width of the whole hexagon (point-to-point) = `2D/sqrt3`
- dx: smallest x delta = W/4 = `D/(2sqrt3)`
- dy: smallest y delta = H/4 = D/4 = `(sqrt3dx)/2`
- Neighbor hexes at:
	- (0, H), (3W/4,H/2), (3W/4,-H/2), (0, -H), (-3W/4,-H/2), (-3W/4, H/2)
	- (0, D), (3D/2sqrt3, D/2), (3D/2sqrt3,-D/2), (0, -D), (-3D/2sqrt3,-D/2), (-3D/2sqrt3, D/2)
	- (0, 4dy), (3dx, 2dy), (3dx, -2dy), (0, -4dy), (-3dx, -2dy), (-3dx, 2dy)
- S: distance from hex center to vertex = `W/2` = `D/sqrt3`
- Vertices relative to center: 
	- (-W/4, H/2), (W/4, H/2), (W/2, 0), (W/4, -H/2), (-W/4, -H/2), (-W/2, 0)
	- (-S/2, sqrt3S/2), (S/2, sqrt3S/2), (S, 0), (S/2, -sqrt3S/2), (-S/2, -sqrt3S/2), (-S, 0)
	- (-D/2sqrt3, D/2), (D/2sqrt3, D/2), (D/sqrt3, 0), (D/2sqrt3, -D/2), (-D/sqrt3, -D/2), (-D/sqrt3, 0)
	- (-dx, 2dy), (dx, 2dy), (2dx, 0), (dx, -2dy), (-dx, -2dy), (-2dx, 0)
## Details

The canonical position of [[Entities]] are either aligned to the center of a hexagon, or to the vertices connecting the hexagons. This means the coordinates of all entities can be represented in quarters of hex widths, and halves of hex heights. For consistency, the (x,y) coordinates of an entity will represent its position in quarter widths and quarter heights from the world origin. All vertices and all hexagon centers will be aligned with an even y.

The distance between vertical layers is S, or `D/sqrt3`.
## Rendering

### Anatomy

A tile contains both static scenery elements and dynamic entities. 

Scenery will often include a floor, which may have a visible foundation below. The floor is aligned with the z elevation of the grid, and usually characters that stand on it will have their feet on the floor. 

Other scenery elements may be attached to the center of the tile, the edges, and the vertices. An attachment in the center of the hexagon is part of the floor: if entity can also occupy the same hexagon, it renders under them. Attachments on the bottom three edges or bottom two vertices render in front of entities on the hexagon, attachments on the top three edges or two top vertices render behind entities on the hexagon. Other scenarios are undefined, and should probably be avoided through game constraints or limitations, or else rendering may be non-deterministic.

In rendering order for a single z-layer, we get:
- Floors
- Top attachments (top edge and two top vertices, then the upper diagonal edges)
- Center attachments
- Occupying Entities
- Bottom attachments (lower diagonal edges, then the bottom edge and the two bottom vertices)

Rendering may be skipped if known to not be visible. Foundations should not be expected to be visible if the tile has three immediate southern neighbors, for example.

Hexes are viewed from an "isometric" (dimetric) projection, not directly from above. This distorts the shape of the hexagon further: while in theory the hex is already 2/sqrt3 times as wide as it is tall, the view will further shrink the longitudinal (y) extent of the hex. 

This will however allow the z axis to be visible, with elements such as foundations, walls, and most entities. Some care in rendering will be needed to be able to distinguish entities that are higher on the z axis versus further on the y axis, which will both display as "higher" on the screen display.

### Projection

Viewed from the top, a 32 pixel wide hex would normally be about 27.7 pixels tall (sqrt3W/2). The vertical gameplay elements would not be visible.

With a 45 degree projection, the height becomes 19.6 pixels (27.7cos(45 deg)). The vertical elements that are one z-layer tall would be 11.3 pixels tall (16sin(45)).

To simplify, we're settling on hexagons being 32x20 pixels. Neighboring tiles would be found at (-24, +10), (0, +20), (24, +10), (24, -10), (0, -20), and (-24, -10) pixels. In addition, to get round numbers, we'll treat z-layers as ±12 pixels - this makes a floor and its entire foundation fit neatly in a 32x32 pixel square.

This means that every dx coordinate is a delta of 8 pixels, and every dy coordinate is a delta of 5 pixels. 

Given the overlaps between 32x32 tiles, we need some rule to decide what gets rendered on top between tiles on the same layer, to avoid non-deterministic renders. For this purpose, the top 10 pixels of a tile must always be drawn over the bottom 22 pixels of another tile, which should handle all overlaps. Whether this means the sprites get split in two, or some other algorithm is used to make sure this is true, is left to the implementation. 

Tiles on different layers will always be strictly above or below other layers.
### Assets

On a floor tileset image, each 32x32 block starting from the top-left corner contains a floor graphic. When specifying a floor sprite by index, we traverse from top-left to top-right until we reach the right edge, then indices resume from the left of the next row down.

## Collision

Tiles may prevent entities from occupying them, or traversing to neighboring tiles (ex: walls). 

A tile may have one of the following states:
- Blocked: tile cannot be occupied or overlapped
- Obstructed: tile cannot be occupied, but can be partially accessed
- Free: tile can be occupied or overlapped

A tile occupied by a character counts as blocked for others, but free for the character themself.

Edges can be one of the following states:
- Blocked: cannot be traversed
- Free: can be traversed

To move from one tile to another, the destination tile and the edge between source and destination must be free. 
## References
https://www.redblobgames.com/grids/hexagons/
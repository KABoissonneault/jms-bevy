This document describes the composition of the full scene, which will be rendered into the viewport based on the camera.

## Render Layers

### Background

All background elements render behind all the other layers. This has a Clear Color which serves as the default color when nothing else is rendered over a pixel. This may also have other details based on scene design.

### Game

The default render layer. Most of the elements will be rendered there.

### Overlay

The overlay render layer for non-game elements that draw over everything else. These present information to the player without interacting with the game directly.

### Game Layers

Entities with positions in the game exist on a certain z-layer. Each z-layer has its own hex grid (see [[Tiling]]), and scenery or characters made be attached to one of the grids. 

Within a given z-layer, elements can be divided into bottom and top. Floors are bottom elements, and characters and attachments are top elements. All bottoms elements are under all the top elements.

## Implementation notes

bevy_ui already renders UI on top of everything else, it probably will not need its own RenderLayers or camera.
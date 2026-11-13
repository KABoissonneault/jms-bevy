Entities have distinguishable identities within the game simulation. Not all exist physically in space, and even the ones that do don't necessarily occupy a specific point in space.

## Actor

Entity that occupies a position in space. In [[JMS]], positions are aligned to [[Tiling|positions in a grid]], where a given (x, y, z) coordinate selects the z layer, and (x, y) gives the relative position within the associated grid. 
## Character

A typical human character occupies one hex and the six hexagons around it. They can either casually have both feet spread within their hexagon (casual pose), have one foot in another hex (offset pose), or have both feet spread over two hex distances (guarded pose). In normal balance, the center of mass will be between both feet. Regardless, the entity has a "canonical position" that may be separate for both of its feet or its center of mass: gameplay mechanics will have to determine this canonical position, and the character's relative supports or center will depend on pose and balance.

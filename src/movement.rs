//! Movement rules over the hex map, and the `MoveEntity` command every
//! step goes through (`tiling-architecture-plan.md` Tier 1 step 4).
//!
//! `Tiling.md` §Collision's move rule — the destination tile *and* the
//! shared edge must be free — lives in exactly one place, [`can_traverse`],
//! built on [`can_enter`]'s occupancy rule ("a tile occupied by a character
//! counts as blocked for others, but free for the character themself").
//! [`can_walk`] layers the walking geometry on top of those raw rules: a
//! step lands on the topmost tile of the destination xy and may not cross
//! more than [`MAX_STEP_DZ`] z layers. [`MoveEntity`] validates through
//! [`can_walk`] and, only on success, writes the new [`HexPos`] — the
//! [`HexStorage`] lifecycle observers then swap the occupant entries on
//! their own, so the command only ever touches the source of truth.

use crate::prelude::*;
use bevy::prelude::*;

/// Returns whether the tile has enough clearance for a character to walk on
pub fn can_be_occupied(storage: &HexStorage, pos: GridPos) -> bool {
    let base_layer = pos.layer;
    storage.tile_at(pos).is_some()
        && storage.tile_at(pos.at_layer(base_layer + 1)).is_none()
        && storage.tile_at(pos.at_layer(base_layer + 2)).is_none()
}

/// The position an occupant stepping from `from` toward `direction` lands
/// on: the neighboring xy at the z of the topmost tile covering it
/// (`Tiling.md`'s stacked layers — characters always stand on the highest
/// floor). `None` when there is nothing to land on: a hole at the
/// destination xy, or `from` outside the hex lattice.
pub fn step_landing(storage: &HexStorage, from: GridPos, direction: Direction) -> Option<GridPos> {
    let next_pos = from.neighbor(direction);
    if can_be_occupied(storage, next_pos) {
        return Some(next_pos);
    }
    let base_layer = next_pos.layer;
    if can_be_occupied(storage, next_pos.at_layer(base_layer - 1)) {
        return Some(next_pos);
    }
    if can_be_occupied(storage, next_pos.at_layer(base_layer + 1)) {
        return Some(next_pos);
    }
    return None;
}

/// Whether the hexagon at `pos` can be entered by `for_entity`
/// (`Tiling.md` §Collision): its floor tile must exist and be
/// [`TileState::Free`], and its occupant must be absent or the entity
/// itself — blocked for others, free for self.
pub fn can_enter(storage: &HexStorage, pos: GridPos, tile_states: &Query<&TileState>) -> bool {
    storage.tile_at(pos).is_some_and(|tile| {
        tile_states
            .get(tile)
            .is_ok_and(|&state| state == TileState::Free)
    })
}

/// Whether a step from `from` toward `direction` is allowed — the one
/// place `Tiling.md` §Collision's move rule lives: the shared edge between
/// the two tiles must be free, and the destination tile must be enterable.
pub fn can_traverse(
    storage: &HexStorage,
    from: GridPos,
    direction: Direction,
    tile_states: &Query<&TileState>,
) -> Option<GridPos> {
    let Some(landing) = step_landing(storage, from, direction) else {
        return None;
    };
    if can_enter(storage, landing, tile_states) {
        return Some(landing);
    } else {
        return None;
    }
}

/// Whether `for_entity` can walk a step from `from` toward `direction`,
/// and where it would land.
pub fn can_walk(
    storage: &HexStorage,
    from: GridPos,
    direction: Direction,
    tile_states: &Query<&TileState>,
) -> Option<GridPos> {
    can_traverse(storage, from, direction, tile_states)
}

/// Moves an entity one hex toward `direction`, landing on the topmost tile
/// of the destination xy. All movement goes through this command: it
/// validates the step against [`can_walk`] — the movement rules' choke
/// point — and rejects it without any effect when the step is impossible.
#[derive(Debug)]
pub struct MoveEntity {
    pub entity: Entity,
    pub direction: Direction,
}

impl Command for MoveEntity {
    type Out = ();

    fn apply(self, world: &mut World) {
        let Some(&from) = world.get::<GamePos>(self.entity) else {
            info!("MoveEntity rejected: {} has no position", self.entity);
            return;
        };

        let Some(tile_pos) = from.to_grid() else {
            info!("MoveEntity rejected: {} was not on tile", self.entity);
            return;
        };

        // A read-only validation pass, then the single write of the source
        // of truth; the HexStorage observers swap the occupant entries.
        let mut tile_state = world.query::<&TileState>();
        let storage = world.resource::<HexStorage>();
        let Some(landing) = can_walk(storage, tile_pos, self.direction, &tile_state.query(world))
        else {
            info!(
                "{} cannot step {:?} from {from:?}; move rejected",
                self.entity, self.direction
            );
            return;
        };
        let new_pos = GamePos::from(landing);
        world.entity_mut(self.entity).insert(new_pos);
    }
}

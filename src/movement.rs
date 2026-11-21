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

use bevy::prelude::*;

use crate::components::{Direction, EdgeState, HexPos, Tile, TileEdges, TileState};
use crate::resources::HexStorage;

/// Marks the player-controlled occupant that the `QWEASD` keys steer.
#[derive(Component, Debug, Default)]
pub struct Player;

/// Largest height change, in z layers, a single step may cross.
const MAX_STEP_DZ: i32 = 1;

/// The position an occupant stepping from `from` toward `direction` lands
/// on: the neighboring xy at the z of the topmost tile covering it
/// (`Tiling.md`'s stacked layers — characters always stand on the highest
/// floor). `None` when there is nothing to land on: a hole at the
/// destination xy, or `from` outside the hex lattice.
pub fn step_landing(storage: &HexStorage, from: HexPos, direction: Direction) -> Option<HexPos> {
    let next = from.neighbor(direction)?;
    let (z, _) = storage.topmost_tile_at(next.x, next.y)?;
    Some(next.at_layer(z))
}

/// Whether the hexagon at `pos` can be entered by `for_entity`
/// (`Tiling.md` §Collision): its floor tile must exist and be
/// [`TileState::Free`], and its occupant must be absent or the entity
/// itself — blocked for others, free for self.
pub fn can_enter(
    storage: &HexStorage,
    pos: HexPos,
    for_entity: Entity,
    tile_states: &Query<&TileState>,
) -> bool {
    if storage
        .occupant_at(pos)
        .is_some_and(|occupant| occupant != for_entity)
    {
        return false;
    }
    storage.tile_at(pos).is_some_and(|tile| {
        tile_states
            .get(tile)
            .is_ok_and(|&state| state == TileState::Free)
    })
}

/// Whether a step from `from` toward `direction` is allowed — the one
/// place `Tiling.md` §Collision's move rule lives: the shared edge between
/// the two tiles must be free, and the destination tile must be enterable
/// by `for_entity` ([`can_enter`]). The shared edge exists only between
/// tiles on the same z layer; [`HexStorage::edge_toward`] reports no edge
/// where either side is missing, which rejects the step.
pub fn can_traverse(
    storage: &HexStorage,
    from: HexPos,
    direction: Direction,
    for_entity: Entity,
    edges: &Query<&TileEdges>,
    tile_states: &Query<&TileState>,
) -> bool {
    let Some(destination) = from.neighbor(direction) else {
        return false;
    };
    let edge_free = storage
        .edge_toward(from, direction, edges)
        .is_some_and(|edge| edge == EdgeState::Free);
    edge_free && can_enter(storage, destination, for_entity, tile_states)
}

/// Whether `for_entity` can walk a step from `from` toward `direction`,
/// and where it would land. The landing spot is [`step_landing`]'s topmost
/// tile of the destination xy, reachable only when it sits no more than
/// [`MAX_STEP_DZ`] z layers above or below `from`. Same-layer steps follow
/// the full traversal rule ([`can_traverse`]: free shared edge, free
/// destination); cross-layer steps share no edge, so only [`can_enter`]
/// applies there.
pub fn can_walk(
    storage: &HexStorage,
    from: HexPos,
    direction: Direction,
    for_entity: Entity,
    edges: &Query<&TileEdges>,
    tile_states: &Query<&TileState>,
) -> Option<HexPos> {
    let landing = step_landing(storage, from, direction)?;
    if (landing.z - from.z).abs() > MAX_STEP_DZ {
        return None;
    }
    let reachable = if landing.z == from.z {
        can_traverse(storage, from, direction, for_entity, edges, tile_states)
    } else {
        can_enter(storage, landing, for_entity, tile_states)
    };
    reachable.then_some(landing)
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
        let Some(&from) = world.get::<HexPos>(self.entity) else {
            info!("MoveEntity rejected: {} has no position", self.entity);
            return;
        };
        // A read-only validation pass, then the single write of the source
        // of truth; the HexStorage observers swap the occupant entries.
        let mut edge_state = world.query::<&TileEdges>();
        let mut tile_state = world.query::<&TileState>();
        let storage = world.resource::<HexStorage>();
        let Some(landing) = can_walk(
            storage,
            from,
            self.direction,
            self.entity,
            &edge_state.query(world),
            &tile_state.query(world),
        ) else {
            info!(
                "{} cannot step {:?} from {from:?}; move rejected",
                self.entity, self.direction
            );
            return;
        };
        world.entity_mut(self.entity).insert(landing);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::{on_insert_hex_pos, on_remove_hex_pos};

    /// A minimal world: the storage resource and its lifecycle observers,
    /// so spawned tiles and occupants are indexed as in the game.
    fn world() -> World {
        let mut world = World::new();
        world.insert_resource(HexStorage::default());
        world.add_observer(on_insert_hex_pos);
        world.add_observer(on_remove_hex_pos);
        world
    }

    #[test]
    fn can_enter_needs_a_free_tile_and_no_other_occupant() {
        let mut world = world();

        let open = HexPos::from_grid(0, 0);
        world.spawn((open, Tile { floor: 0 }));
        let walker = world.spawn(open).id();
        let blocked_pos = HexPos::from_grid(1, 0);
        world.spawn((blocked_pos, Tile { floor: 0 }, TileState::Blocked));
        let shared = HexPos::from_grid(2, 0);
        world.spawn((shared, Tile { floor: 0 }));
        let other = world.spawn(shared).id();
        let empty = HexPos::from_grid(3, 0);
        world.spawn((empty, Tile { floor: 0 }));

        let mut tile_state = world.query::<&TileState>();
        let storage = world.resource::<HexStorage>();
        let states = tile_state.query(&world);

        assert!(can_enter(storage, empty, walker, &states));
        // Blocked for others, free for self.
        assert!(can_enter(storage, open, walker, &states));
        assert!(!can_enter(storage, open, other, &states));
        assert!(can_enter(storage, shared, other, &states));
        // Blocked tiles, holes, and vertices are never enterable.
        assert!(!can_enter(storage, blocked_pos, walker, &states));
        assert!(!can_enter(
            storage,
            HexPos::from_grid(4, 0),
            walker,
            &states
        ));
        assert!(!can_enter(storage, HexPos::new(2, 0, 0), walker, &states));
    }

    #[test]
    fn can_traverse_needs_a_free_edge_and_a_free_destination() {
        let mut world = world();
        let from = HexPos::from_grid(0, 0);
        let north = from.neighbor(Direction::North).unwrap();
        let from_tile = world.spawn((from, Tile { floor: 0 })).id();
        world.spawn((north, Tile { floor: 0 }));
        let walker = world.spawn(from).id();

        // Free edge, free destination.
        {
            let mut edge_state = world.query::<&TileEdges>();
            let mut tile_state = world.query::<&TileState>();
            let storage = world.resource::<HexStorage>();
            let (edges, states) = (edge_state.query(&world), tile_state.query(&world));
            assert!(can_traverse(
                storage,
                from,
                Direction::North,
                walker,
                &edges,
                &states
            ));
            // A step toward a hole has no edge and no destination.
            assert!(!can_traverse(
                storage,
                from,
                Direction::NorthEast,
                walker,
                &edges,
                &states
            ));
        }

        // Destination occupancy: blocked for others, free for the occupant.
        let other = world.spawn(north).id();
        {
            let mut edge_state = world.query::<&TileEdges>();
            let mut tile_state = world.query::<&TileState>();
            let storage = world.resource::<HexStorage>();
            let (edges, states) = (edge_state.query(&world), tile_state.query(&world));
            assert!(!can_traverse(
                storage,
                from,
                Direction::North,
                walker,
                &edges,
                &states
            ));
            assert!(can_traverse(
                storage,
                from,
                Direction::North,
                other,
                &edges,
                &states
            ));
        }

        // A blocked shared edge rejects everyone, even the destination's
        // own occupant.
        world.entity_mut(from_tile).insert(TileEdges {
            north: EdgeState::Blocked,
            ..TileEdges::FREE
        });
        {
            let mut edge_state = world.query::<&TileEdges>();
            let mut tile_state = world.query::<&TileState>();
            let storage = world.resource::<HexStorage>();
            let (edges, states) = (edge_state.query(&world), tile_state.query(&world));
            assert!(!can_traverse(
                storage,
                from,
                Direction::North,
                walker,
                &edges,
                &states
            ));
            assert!(!can_traverse(
                storage,
                from,
                Direction::North,
                other,
                &edges,
                &states
            ));
        }
    }

    #[test]
    fn can_walk_lands_on_the_topmost_tile_within_one_layer() {
        let mut world = world();
        let from = HexPos::from_grid(0, 0);
        let north = from.neighbor(Direction::North).unwrap();
        world.spawn((from, Tile { floor: 0 }));
        world.spawn((north, Tile { floor: 0 }));
        world.spawn((north.at_layer(1), Tile { floor: 0 }));
        let walker = world.spawn(from).id();

        // One layer up is fine, and lands on the top floor.
        {
            let mut edge_state = world.query::<&TileEdges>();
            let mut tile_state = world.query::<&TileState>();
            let storage = world.resource::<HexStorage>();
            let (edges, states) = (edge_state.query(&world), tile_state.query(&world));
            assert_eq!(
                can_walk(storage, from, Direction::North, walker, &edges, &states),
                Some(north.at_layer(1))
            );
        }

        // With a third floor stacked above, the topmost tile is two layers
        // up — more height than one step may cross.
        let top = world.spawn((north.at_layer(2), Tile { floor: 0 })).id();
        {
            let mut edge_state = world.query::<&TileEdges>();
            let mut tile_state = world.query::<&TileState>();
            let storage = world.resource::<HexStorage>();
            let (edges, states) = (edge_state.query(&world), tile_state.query(&world));
            assert_eq!(
                can_walk(storage, from, Direction::North, walker, &edges, &states),
                None
            );
        }
        world.despawn(top);

        // An occupant camping the top tile leaves no free top-tile.
        world.spawn(north.at_layer(1));
        {
            let mut edge_state = world.query::<&TileEdges>();
            let mut tile_state = world.query::<&TileState>();
            let storage = world.resource::<HexStorage>();
            let (edges, states) = (edge_state.query(&world), tile_state.query(&world));
            assert_eq!(
                can_walk(storage, from, Direction::North, walker, &edges, &states),
                None
            );
        }
    }

    #[test]
    fn can_walk_steps_down_within_one_layer() {
        let mut world = world();
        let ledge = HexPos::from_grid(0, 1);
        let below = ledge.neighbor(Direction::South).unwrap();
        world.spawn((ledge.at_layer(1), Tile { floor: 0 }));
        world.spawn((below, Tile { floor: 0 }));
        let walker = world.spawn(ledge.at_layer(1)).id();

        let mut edge_state = world.query::<&TileEdges>();
        let mut tile_state = world.query::<&TileState>();
        let storage = world.resource::<HexStorage>();
        let (edges, states) = (edge_state.query(&world), tile_state.query(&world));
        assert_eq!(
            can_walk(
                storage,
                ledge.at_layer(1),
                Direction::South,
                walker,
                &edges,
                &states
            ),
            Some(below)
        );
    }

    #[test]
    fn move_entity_command_moves_and_rejects_without_effects() {
        let mut world = world();
        let from = HexPos::from_grid(0, 0);
        let north = from.neighbor(Direction::North).unwrap();
        world.spawn((from, Tile { floor: 0 }));
        world.spawn((north, Tile { floor: 0 }));
        let walker = world.spawn(from).id();

        world.commands().queue(MoveEntity {
            entity: walker,
            direction: Direction::North,
        });
        world.flush();

        assert_eq!(world.get::<HexPos>(walker), Some(&north));
        let storage = world.resource::<HexStorage>();
        assert_eq!(storage.occupant_at(from), None);
        assert_eq!(storage.occupant_at(north), Some(walker));

        // A step onto a hole changes nothing.
        world.commands().queue(MoveEntity {
            entity: walker,
            direction: Direction::North,
        });
        world.flush();
        assert_eq!(world.get::<HexPos>(walker), Some(&north));

        // A command for an entity without a position is a harmless no-op.
        let ghost = world.spawn(()).id();
        world.commands().queue(MoveEntity {
            entity: ghost,
            direction: Direction::South,
        });
        world.flush();
        assert_eq!(world.get::<HexPos>(ghost), None);
    }
}

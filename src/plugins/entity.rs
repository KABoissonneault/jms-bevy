use crate::prelude::*;
use bevy::prelude::*;
use bevy::sprite::Anchor;

/// Portion of the character sprite that hangs below its hexagon center: the
/// character's feet, resting on the floor of the tile it stands on.
const CHARACTER_FEET_FRACTION: f32 = 0.16;
/// Anchor putting that point of the sprite on the hexagon center, instead
/// of the image's middle (`Anchor::CENTER`).
const CHARACTER_SPRITE_ANCHOR: Anchor = Anchor(Vec2::new(0.0, -0.5 + CHARACTER_FEET_FRACTION));

/// Sprite of the reference occupant standing on the test map.
const CAPSULE_SPRITE_PATH: &str = "sprites/capsule.png";

pub(crate) fn plugin(app: &mut App) {
    app.add_systems(Update, spawn_player)
        .add_systems(Last, sync_dynamic_entity_transforms);
}

/// Characters are above its z-layer, and above the northern neighbors of the above z-layer, but below the southern neighbors
const CHARACTER_Z_OFFSET: f32 = 1.0;

fn get_character_translation(pos: GamePos) -> Vec3 {
    let delta = pixel_delta(pos);
    let depth = get_coord_depth(pos) + CHARACTER_Z_OFFSET;
    Vec3::new(delta.x, delta.y, depth)
}

fn spawn_player(
    mut commands: Commands,
    storage: Res<HexStorage>,
    assets: Res<AssetServer>,
    spawn: Query<&GridPos>,
    mut spawned: Local<bool>,
) {
    if *spawned {
        return;
    }

    let spot = spawn
        .iter()
        .copied()
        .find(|pos| can_be_occupied(&storage, *pos));

    let Some(spot) = spot else {
        return; // No tiles on the map yet — try again next frame.
    };

    let game_pos = GamePos::from(spot);

    *spawned = true;

    commands.spawn((
        Name::new("Player"),
        game_pos,
        Player,
        Sprite {
            image: assets.load(CAPSULE_SPRITE_PATH),
            ..default()
        },
        CHARACTER_SPRITE_ANCHOR,
        Transform::from_translation(get_character_translation(game_pos)),
    ));
}

fn sync_dynamic_entity_transforms(
    mut occupants: Query<
        (&GamePos, &mut Transform, Option<&mut Mesh2d>),
        (With<DynamicPosEntity>, Changed<GamePos>),
    >,
) {
    for (pos, mut transform, mesh) in &mut occupants {
        transform.translation = get_character_translation(*pos);
        // Workaround for https://github.com/bevyengine/bevy/issues/26362
        // Fixed in https://github.com/bevyengine/bevy/pull/26371, should be removed after v0.20.1
        if let Some(mut mesh) = mesh {
            mesh.set_changed();
        }
    }
}

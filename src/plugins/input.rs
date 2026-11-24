#![allow(unused)]

use crate::prelude::*;
use bevy::prelude::*;

/// The `QWEASD` movement keys of `Tiling.md`, paired with the hex
/// direction each one steps: Q `NorthWest`, W `North`, E `NorthEast`, A
/// `SouthWest`, S `South`, D `SouthEast`.
const MOVE_KEYS: [(KeyCode, Direction); 6] = [
    (KeyCode::KeyQ, Direction::NorthWest),
    (KeyCode::KeyW, Direction::North),
    (KeyCode::KeyE, Direction::NorthEast),
    (KeyCode::KeyA, Direction::SouthWest),
    (KeyCode::KeyS, Direction::South),
    (KeyCode::KeyD, Direction::SouthEast),
];

#[derive(Resource)]
pub struct MousePosition(Vec2);

pub(crate) fn plugin(app: &mut App) {
    app.insert_resource(MousePosition(Vec2::default()))
        .add_systems(Update, move_player_on_input);
}

/// Steps the player one hex per `QWEASD` press. Movement itself goes
/// through [`MoveEntity`], which validates the step against the movement
/// rules and rejects it without effects when the destination has no free
/// topmost tile or the height change crosses more than one z layer.
fn move_player_on_input(
    input: Res<ButtonInput<KeyCode>>,
    player: Single<Entity, With<Player>>,
    mut commands: Commands,
) {
    for (key, direction) in MOVE_KEYS {
        if input.just_pressed(key) {
            commands.queue(MoveEntity {
                entity: *player,
                direction,
            });
        }
    }
}

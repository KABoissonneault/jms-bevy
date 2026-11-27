use crate::prelude::*;
use bevy::prelude::*;

// Components for entity type identification

// Entity whose position will change dynamically during gameplay
dev_reflect_component! {
    #[derive(Component, Default)]
    pub struct DynamicPosEntity;
}

// Entities of type Character
dev_reflect_component! {
    #[derive(Component, Default)]
    #[require(DynamicPosEntity)]
    pub struct Character;
}

// The entity being controlled by the player
dev_reflect_component! {
    #[derive(Component, Default)]
    #[require(Character)]
    pub struct Player;
}

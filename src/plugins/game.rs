#![allow(unused)]

use bevy::{app::App, prelude::*, sprite::Anchor};

use crate::styles::colors;

// This is an example of the most simple plugin you can write, without
// having to implement any traits.
//
// If you wanted to toggle this plugin or configure it for the outside
// you can reach for a `PluginGroup`.

pub(crate) fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn_title);
}

fn spawn_title(mut commands: Commands) {
    commands.spawn((
        Name::new("Title"),
        Text2d::new("Je me souviens"),
        TextFont::from_font_size(FontSize::Vw(10.0)),
        TextColor(colors::TITLE_TEXT.into()),
        TextLayout::justify(Justify::Center),
        Anchor::CENTER,
    ));
}

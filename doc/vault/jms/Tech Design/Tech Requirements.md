This page aims to list the high-level expected features and their associated tech requirements. This is a reference to help establish the fundamental technology used by the project and how to configure them.

The JMS project is a single player roleplaying game. The actions are turn-based and discrete (ex: aligned to tiles in a grid). The game will feature audio in the form of minimal sound effects and ambient tracks. The visuals will be represented by 2d pixel art, though likely arrayed to form an isometric perspective. 

## Programming

The software should be mostly programmed in Rust. Rust is a robust low-overhead language, with a rich ecosystem of libraries and tools. Its statically-enforced requirements and strongly opinionated patterns make it a good fit for agents. 

The project will leverage the Bevy game engine. While Bevy does not come with much useful tools such as an editor, it does come with much of essential components already implemented and tested, and its idioms and practices have many references online compared to starting from scratch. Many plugins add functionality beyond the base engine.

The project will try to keep at head with the latest Bevy version that is supported by all the plugins.

## Target Platforms

The target platforms will be native desktop and web.

## Gameplay 

Most of the gameplay occurs with entities moving on a tile grid. More information in the [[Tiling]] document. 

Each turn, the player can take as much time as they want to choose an action, and when they do, the entire world reacts simultaneously. These actions can include movement, attacks, dialogue, and other skill checks.

## Graphics

Visuals will be using only 2d renders, though potentially arranged in an isometric perspective to display multiple perspectives at the same time.

Assets will be low resolution pixel art.

## Audio

The game will contain ambient tracks and sound effects. Likely on the lo-fi end of things. Possibly MIDI.

## Controls

The game should be fully playable with just a keyboard. The mouse may be used for certain interface elements.

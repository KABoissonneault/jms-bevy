This page documents the development process of jms-bevy.

jms-bevy is an implementation of the [[JMS]] project idea using Rust and the Bevy game engine. 

The project is also attempting to enforce a specific agentic **coding** workflow. 

## Introduction

For a game project, many elements, such as the visuals, the sound design, the narrative, the gameplay formulas, etc, are an artistic product: the intent and the process are the purpose. The end user cares that all these decisions were made and implemented with intention. Code, however, is a mean to an end. Our expectation is that it does what is intended, nothing more, nothing less. Certain coding competitions like code golf, Advent of Code, or demo scenes might care about the process, but for everything else, it is just the wires behind the panel.  Many games are developed without code using high-level engines. 

With this in mind, we accept that delegating the implementation of a technical design to a statistical model does not compromise "the point" of making a game. Choosing what technical problems need being addressed, and what architecture will best represent those problems and how those problems evolve over time, is still left to a human engineer, because it matters more to the player, because models are just not as good at predicting an evolving specification as human engineers, and because it is the more fun part of making software (for some).

Definitively, the development process must have a human writing all the documents here for them to have any meaning. The documents here will guide the maintainers, the coding agents, and the casual inspector trying to learn more about the development of the project. The project should limit the amount of code that is not represented by something written here with intention.

The same holds for any other non-code assets used by the game. There is no point in getting a machine to make better looking visuals while the maintainers are not able or willing to put more effort into them. The visuals, the sounds, the narrative, and other perceptible elements of the game should match the intent and capabilities of the artist. 

We choose agents for code not because it is trendy, or because it is faster, or because the maintainers lost all passion for coding. We choose it because we believe it can often be the best tool for the job.

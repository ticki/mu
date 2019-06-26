//! Mu -- Advanced Unix-style Memory Card System

// TODO: Clean up `use`
// TODO: Run rustfmt
// TODO: Add terminology overview and go over all the documentation again
// TODO: Fix links in rustdoc.
// TODO: Configurable key bindings

extern crate chrono;
extern crate rand;
extern crate serde_yaml as yaml;

mod settings;
mod deck;
mod cards;
mod scheduler;

pub use deck::Deck;
pub use cards::{Card, CardState, Score, Metacard, Review, Command, View, SCORES};
pub use scheduler::{Schedule, Scheduler, Statistics};

/// A point in time.
type Time = chrono::DateTime<chrono::Utc>;

/// Get current time.
fn now() -> Time {
    chrono::Utc::now()
}

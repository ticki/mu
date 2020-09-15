//! Content and state of flashcards.

use std::{env, process, io, cmp, fmt};

use chrono;
use serde::{Serialize, Deserialize};

use crate::{now, settings, Time};

// TODO: Remove all comments in this file and write new comments to explain the details of the
//       formulas.

// TODO: Get rid of this when https://github.com/chronotope/chrono/issues/117 is closed.
/// A helper structure for serialization.
#[derive(Serialize, Deserialize)]
#[serde(remote = "chrono::Duration")]
struct DurationDef {
    #[serde(getter = "chrono::Duration::num_seconds")]
    secs: i64,
}

impl From<DurationDef> for chrono::Duration {
    fn from(def: DurationDef) -> chrono::Duration {
        chrono::Duration::seconds(def.secs)
    }
}

/// The ease of a card.
///
/// The updated interval of the card is, under normal circumstances (i.e. learned cards),
/// multiplied by this factor.
pub type Ease = f32;
/// The identifier of a card, as is specfied by the user in the card file.
pub type CardId = String;
/// A card's priority; takes a value 0-4.
pub type Priority = u8;

/// The number of priority levels.
pub const PRIORITIES: usize = 5;
/// The number of scores.
pub const SCORES: usize = 5;

/// The user-specified score of the review of a single card.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
pub enum Score {
    /// Fail; the card will go into relearn state.
    Fail = 0,
    /// Success but hard.
    Hard = 1,
    /// Success at the right level of difficulty.
    Okay = 2,
    /// Success at a good level of difficulty.
    Good = 3,
    /// Success at a too easy level of difficulty.
    Easy = 4,
}

impl fmt::Display for Score {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Score::Fail => write!(f, "fail"),
            Score::Hard => write!(f, "hard"),
            Score::Okay => write!(f, "okay"),
            Score::Good => write!(f, "good"),
            Score::Easy => write!(f, "easy"),
        }
    }
}

/// A card's state.
#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub enum CardState {
    /// A new, unreviewed card.
    New,
    /// Learning; at the specified step.
    Learning(usize),
    /// Relearning; at the specified step.
    Relearning(usize),
    /// Has already been learnt.
    Learnt,
}

/// A card review.
///
/// This is used to track the history of a card.
#[derive(Serialize, Deserialize, Debug)]
pub struct Review {
    /// The time of the review.
    pub time: Time,
    /// The time at which the card was due.
    pub due: Time,
    /// The user-specified score.
    pub score: Score,
    /// The interval that was ended.
    #[serde(with = "DurationDef")]
    pub ended_interval: chrono::Duration,
    /// The state of the card before review.
    pub state_before: CardState,
    /// The ease of the card before review.
    pub ease_before: Ease,
}

/// Data for viewing a card.
#[derive(Debug)]
pub enum View {
    /// A PDF-file.
    Pdf(String),
    /// A `sh` (hopefully POSIX) command.
    Command(Command),
}

impl View {
    pub fn as_str(&self) -> &str {
        match self {
            View::Pdf(ref s) => s,
            View::Command(_) => "[command]",
        }
    }
}

/// A `sh` (hopefully POSIX) command.
#[derive(Debug)]
pub struct Command(pub String);

impl Command {
    /// Execute the command.
    ///
    /// For good measure, we set the `$CARD_ID` environment variable to contain the card ID before
    /// execution.
    pub fn execute(&self) -> io::Result<process::Child> {
        // Set the `$CARD_ID` environment variable, such that the script, we execute can tell what
        // card it is.
        env::set_var("CARD_ID", &self.0);

        // Run the command in a shell.
        process::Command::new("/bin/sh")
            .arg("-c")
            .arg(&self.0)
            .spawn()
    }
}

/// The user specfied content of a card.
#[derive(Debug)]
pub struct Card {
    /// The ways the card shall be viewed (e.g. paths to PDFs).
    pub view: Vec<View>,
    /// The tags of the card.
    pub tags: Vec<String>,
    /// The card's priority.
    pub priority: Priority,
    /// The user-specified upper-bound for the interval of the card.
    ///
    /// If the calculated interval exceeds bound, the given interval will be this duration.
    pub max_interval: chrono::Duration,
}

impl Default for Card {
    fn default() -> Card {
        Card {
            view: Vec::new(),
            tags: Vec::new(),
            // 2 should be around the average priority, so seems like a good default value.
            priority: 2,
            // Default to no maximal interval.
            max_interval: chrono::Duration::max_value(),
        }
    }
}

/// The background information and state of a flashcard.
///
/// The content of the card is stored separately, in the form of a `Card`.
#[derive(Serialize, Deserialize, Debug)]
pub struct Metacard {
    /// The user-specified ID of the card.
    pub id: CardId,
    /// The card's state.
    pub state: CardState,
    /// The card's current interval.
    #[serde(with = "DurationDef")]
    pub current_interval: chrono::Duration,
    /// Next time the card will be reviewed.
    pub due: Time,
    /// The reviews of the card.
    pub history: Vec<Review>,
    /// The ease of the card.
    pub ease: Ease,
}

impl Metacard {
    pub fn new(id: CardId, settings: &settings::TagSettings) -> Metacard {
        Metacard {
            id,
            state: CardState::New,
            current_interval: chrono::Duration::zero(),
            due: now(),
            history: Vec::new(),
            ease: settings.starting_ease,
        }
    }

    /// Calculate new interval assuming that `self.state` is `New`.
    fn new_interval_new(&self, settings: &settings::TagSettings, score: Score, max_interval: chrono::Duration) -> chrono::Duration {
        // Cap at maximal interval.
        cmp::min(max_interval, settings.learning_intervals[settings.get_learning_interval(
            settings.learning_interval_progressions[score as usize] - 1
        )])
    }

    /// Calculate new interval assuming that `self.state` is `Learning(step)`.
    fn new_interval_learning(&self, settings: &settings::TagSettings, score: Score, step: usize, max_interval: chrono::Duration)
        -> chrono::Duration
    {
        // Cap at maximal interval.
        cmp::min(max_interval, settings.learning_intervals[settings.get_learning_interval(
            step as isize + settings.learning_interval_progressions[score as usize]
        )])
    }

    /// Calculate new interval assuming that `self.state` is `Relearning(step)`.
    fn new_interval_relearning(&self, settings: &settings::TagSettings, score: Score, step: usize, max_interval: chrono::Duration)
        -> chrono::Duration
    {
        // Cap at maximal interval.
        cmp::min(max_interval, settings.relearning_intervals[settings.get_relearning_interval(
            step as isize + settings.relearning_interval_progressions[score as usize]
        )])
    }

    /// Calculate new interval after nonfailed review assuming that `self.state` is `Learnt`.
    fn new_interval_learnt(&self, settings: &settings::TagSettings, score: Score, priority: Priority, modifier: Ease, max_interval: chrono::Duration) -> chrono::Duration {
        debug_assert!(score != Score::Fail);

        // Calculate unsaturated new interval. This formula is based on the SM2 algorithm.
        let new_int = chrono::Duration::minutes((self.current_interval.num_minutes() as f32
            * self.new_ease(settings, score)
            * modifier
            * settings.interval_modifier
            * settings.score_modifiers[score as usize]
            * settings.priority_modifiers[priority as usize]) as i64);

        // Saturate the new interval according to the chosen settings.
        cmp::min(max_interval, if new_int > settings.max_interval {
            settings.max_interval
        } else if new_int - self.current_interval < settings.min_interval_increase {
            self.current_interval + settings.min_interval_increase
        } else {
            new_int
        })
    }

    /// Calculate the new ease after reviewing a card with score `score`.
    fn new_ease(&self, settings: &settings::TagSettings, score: Score) -> Ease {
        // Add value to the ease.
        let new_ease = self.ease + settings.ease_increase[score as usize];

        // Saturate if necessary.
        if new_ease < settings.min_ease {
            settings.min_ease
        } else if new_ease > settings.max_ease {
            settings.max_ease
        } else {
            new_ease
        }
    }

    /// The updated intervals, depending on score.
    pub fn new_intervals(&self, settings: &settings::TagSettings, priority: Priority, modifier: Ease, max_interval: chrono::Duration) -> [chrono::Duration; SCORES] {
        match self.state {
            CardState::New => [
                self.new_interval_new(settings, Score::Fail, max_interval),
                self.new_interval_new(settings, Score::Hard, max_interval),
                self.new_interval_new(settings, Score::Okay, max_interval),
                self.new_interval_new(settings, Score::Good, max_interval),
                self.new_interval_new(settings, Score::Easy, max_interval),
            ],
            CardState::Learning(step) => [
                self.new_interval_learning(settings, Score::Fail, step, max_interval),
                self.new_interval_learning(settings, Score::Hard, step, max_interval),
                self.new_interval_learning(settings, Score::Okay, step, max_interval),
                self.new_interval_learning(settings, Score::Good, step, max_interval),
                self.new_interval_learning(settings, Score::Easy, step, max_interval),
            ],
            CardState::Relearning(step) => [
                self.new_interval_relearning(settings, Score::Fail, step, max_interval),
                self.new_interval_relearning(settings, Score::Hard, step, max_interval),
                self.new_interval_relearning(settings, Score::Okay, step, max_interval),
                self.new_interval_relearning(settings, Score::Good, step, max_interval),
                self.new_interval_relearning(settings, Score::Easy, step, max_interval),
            ],
            CardState::Learnt => [
                // If the card was failed, we enter relearning.
                settings.relearning_intervals[0],
                self.new_interval_learnt(settings, Score::Hard, priority, modifier, max_interval),
                self.new_interval_learnt(settings, Score::Okay, priority, modifier, max_interval),
                self.new_interval_learnt(settings, Score::Good, priority, modifier, max_interval),
                self.new_interval_learnt(settings, Score::Easy, priority, modifier, max_interval),
            ],
        }
    }

    /// Update the card after review.
    pub fn review(&mut self, settings: &settings::TagSettings, score: Score, priority: Priority, familiarity: Ease, max_interval: chrono::Duration) {
        let now = now();
        // Add review to card history.
        self.history.push(Review {
            time: now,
            due: self.due,
            ended_interval: self.current_interval,
            score,
            state_before: self.state,
            ease_before: self.ease,
        });

        // Update the card state.
        match self.state {
            CardState::New => {
                // Update interval.
                self.current_interval = self.new_interval_new(settings, score, max_interval);
                self.due = now + self.current_interval;
                // Update state.
                self.state = CardState::Learning(0);
            },
            CardState::Learning(step) => {
                // Update interval.
                self.current_interval = self.new_interval_learning(settings, score, step, max_interval);
                self.due = now + self.current_interval;

                // TODO: Get rid of this spaghetti. This is already a done in
                //       `new_interval_learning`. For example, make `new_interval_learning` also
                //       return `new_step`.
                // Calculate new step.
                let new_step = settings.get_learning_interval(
                    step as isize + settings.learning_interval_progressions[score as usize]
                );

                // Update state.
                self.state = if new_step == settings.learning_intervals.len() - 1 {
                    CardState::Learnt
                } else {
                    CardState::Learning(new_step)
                };
            },
            CardState::Relearning(step) => {
                // TODO: Get rid of this spaghetti. This is already a done in
                //       `new_interval_relearning`. For example, make `new_interval_relearning` also
                //       return `new_step`.
                // Update interval.
                self.current_interval = self.new_interval_relearning(settings, score, step, max_interval);
                self.due = now + self.current_interval;

                // Calculate new step.
                let new_step = settings.get_relearning_interval(
                    step as isize + settings.relearning_interval_progressions[score as usize]
                );

                // Update state.
                self.state = if new_step == settings.relearning_intervals.len() - 1 {
                    CardState::Learnt
                } else {
                    CardState::Relearning(new_step)
                };
            },
            CardState::Learnt => {
                if score == Score::Fail {
                    // Update state if card failed.
                    self.state = CardState::Relearning(0);
                    // Update interval.
                    self.current_interval = settings.relearning_intervals[0];
                    self.due = now + self.current_interval;
                } else {
                    // Update interval.
                    self.current_interval = self.new_interval_learnt(settings, score, priority, familiarity, max_interval);
                    self.due = now + self.current_interval;
                }
                // Update ease.
                self.ease = self.new_ease(settings, score);
            },
        }
    }
}

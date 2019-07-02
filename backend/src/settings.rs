//! User customization and settings.

use crate::cards;

/// Global settings.
#[derive(Debug)]
pub struct GlobalSettings {
    /// The maximal length that the queue of to-be-introduced cards may have.
    pub max_new_queue: usize,
    /// The maximal number of unlearnt cards that can be introduced on a day.
    pub max_new_daily: usize,
}

impl Default for GlobalSettings {
    fn default() -> GlobalSettings {
        GlobalSettings {
            max_new_queue: 20,
            max_new_daily: 10,
        }
    }
}

/// Settings for a tag.
#[derive(Clone, Debug)]
pub struct TagSettings {
    /// The intervals that need to be cycled through before a card is learnt.
    ///
    /// The last entry specifies the initial interval (without any modification) for the card after
    /// it has been learned.
    ///
    /// The first entry specifies the interval after the first review of the card.
    pub learning_intervals: Vec<chrono::Duration>,
    /// The progressions in the learning interval for the various scores.
    ///
    /// When a card that is being learnt has been reviewed, the user's choice of their score
    /// (specified with `Score`) affects how much the card closes in on becoming "learnt". In
    /// particular, this array tells how much a certain score progresses in learning intervals
    /// (specified by `learning_intervals`). For example, if the second entry was 1, then choosing
    /// `Hard` would make the card go to the next learning interval. It saturates upon going out of
    /// bounds, so one might for example set `-1000` to go to the first interval.
    pub learning_interval_progressions: [isize; cards::SCORES],
    /// The intervals that need to be cycled through before a card is relearnt.
    ///
    /// Acts like `learning_intervals` but for cards that were failed.
    pub relearning_intervals: Vec<chrono::Duration>,
    /// The progressions in the relearning interval for various scores.
    ///
    /// Acts like `relearning_interval_progression` but for cards that were failed.
    pub relearning_interval_progressions: [isize; cards::SCORES],
    /// The maximal possible interval.
    ///
    /// If the next calculated interval is greater than this, it is saturated to this interval.
    pub max_interval: chrono::Duration,
    /// The minimal possible increase an interval of a learned card can get.
    ///
    /// If the next calculated interval increases by a value smaller than this, the current
    /// interval is instead increased by this value.
    pub min_interval_increase: chrono::Duration,
    /// The default, initial ease a card has when becoming learnt.
    pub starting_ease: cards::Ease,
    /// The maximal value the ease can be.
    pub max_ease: cards::Ease,
    /// The minimal value the ease can be.
    pub min_ease: cards::Ease,
    /// The increase in ease for each specified score.
    pub ease_increase: [cards::Ease; cards::SCORES],
    /// A factor that any new interval will be multiplied by.
    ///
    /// If you retention rate is too high (or low), you should increase (or respectively reduce)
    /// this number.
    pub interval_modifier: f32,
    /// Factors that intervals after review are multiplied by depending on the score.
    ///
    /// As failing makes the card go into relearning mode, the first entry has no effect.
    pub score_modifiers: [f32; cards::SCORES],
    /// Factors that intervals after review are multiplied by depending on the card's priority.
    pub priority_modifiers: [f32; cards::PRIORITIES],
    /// The weight given to the new score when calculating the new adaptive retention rate.
    pub score_weight: f32,
    /// The base change in familiarity for associated tags after each review.
    pub familiarity_delta: f32,
    /// The maximal value for familiarity.
    pub max_familiarity: f32,
    /// The minimal value for familiarity.
    pub min_familiarity: f32,
    /// The ideal retention rate.
    ///
    /// This is used for adaptiveness, particularly for updating tag-level ease.
    pub desired_retention_rate: f32,
}

impl TagSettings {
    /// Gets index to the `n`'th learning interval (in `learning_intervals`), saturating on out-of-bounds.
    pub fn get_learning_interval(&self, n: isize) -> usize {
        if n <= 0 {
            0
        } else if n < self.learning_intervals.len() as isize {
            n as usize
        } else {
            self.learning_intervals.len() - 1
        }
    }

    /// Gets index to the `n`'th relearning interval (in `relearning_intervals`), saturating on out-of-bounds.
    pub fn get_relearning_interval(&self, n: isize) -> usize {
        if n <= 0 {
            0
        } else if n < self.relearning_intervals.len() as isize {
            n as usize
        } else {
            self.relearning_intervals.len() - 1
        }
    }
}

impl Default for TagSettings {
    fn default() -> TagSettings {
        TagSettings {
            learning_intervals: vec![
                chrono::Duration::days(1),
                chrono::Duration::days(2),
                chrono::Duration::days(3),
            ],
            learning_interval_progressions: [-2, 1, 1, 2, 2],
            relearning_intervals: vec![
                chrono::Duration::minutes(30),
                chrono::Duration::days(1),
            ],
            relearning_interval_progressions: [-999, 1, 1, 1, 2],
            max_interval: chrono::Duration::days(50),
            min_interval_increase: chrono::Duration::days(1),
            starting_ease: 2.5,
            max_ease: 3.5,
            min_ease: 1.3,
            ease_increase: [-0.2, -0.15, 0.0, 0.05, 0.15],
            interval_modifier: 1.0,
            score_modifiers: [1.0, 0.7, 1.0, 1.2, 1.4],
            priority_modifiers: [3.0, 2.0, 1.0, 0.5, 0.3],
            score_weight: 0.05,
            familiarity_delta: 0.1,
            max_familiarity: 2.0,
            min_familiarity: 0.4,
            desired_retention_rate: 0.82,
        }
    }
}

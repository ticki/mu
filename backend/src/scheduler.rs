//! Scheduling of memory cards.

use std::cmp;
use std::collections::{BTreeMap, HashMap, HashSet};

use rand::Rng;
use serde::{Serialize, Deserialize};
use chrono;
use yaml;

use crate::{now, cards, settings, deck, Time};

// TODO: Proper handling of potential OOB errors happening when you mess up `Schedule` store. For
//       example, what if you had too many or too few metacards. What if there were no metacards?
//       etc

/// The identifier of a metacard, corresponding to an entry in `Schedule.metacards`.
type MetacardRef = usize;

/// Review statistics about a group of cards.
#[derive(Serialize, Deserialize, Debug)]
pub struct Statistics {
    /// The scores of reviews in chronological order.
    reviews: Vec<cards::Score>,
    /// The number of cards that were reviewed on certain days.
    // TODO: Change to `chrono::Date`, when it eventually implements `Serialize` upstream.
    activity: HashMap<chrono::naive::NaiveDate, u64>,
    /// An average of scores weighted after recency.
    ///
    /// This approximately takes values between 0 and 1.
    adaptive_retention_rate: f32,
    /// High-level ease for this group of cards.
    familiarity: cards::Ease,
}

impl Statistics {
    /// Create default, empty statistics.
    pub fn new(settings: &settings::TagSettings) -> Statistics {
        Statistics {
            reviews: Vec::new(),
            activity: HashMap::new(),
            adaptive_retention_rate: settings.desired_retention_rate,
            familiarity: 1.0,
        }
    }

    /// Calculate the retention rate.
    pub fn retention_rate(&self) -> f32 {
        self.reviews.iter().filter(|&&x| x != cards::Score::Fail).count() as f32
            / self.reviews.len() as f32
    }

    /// Add a review of certain score to the statistics.
    fn review(&mut self, score: cards::Score, settings: &settings::TagSettings) {
        // Record review.
        self.reviews.push(score);
        // Record activity.
        *self.activity.entry(now().date().naive_utc()).or_insert(0) += 1;
        // Update the performance average, taking the weighted average of the old average and a
        // value associated to the new score.
        self.adaptive_retention_rate = (1.0 - settings.score_weight) * self.adaptive_retention_rate
            + settings.score_weight * match score {
            cards::Score::Fail => 0.0,
            cards::Score::Hard => 0.95,
            cards::Score::Okay => 1.0,
            cards::Score::Good => 1.02,
            cards::Score::Easy => 1.05,
        };

        // Increase/decrease the increase if the adaptive retention rate is better/worse than the
        // desired retention rate.
        if self.adaptive_retention_rate >= settings.desired_retention_rate {
            // Increase familiarity by the delta.
            self.familiarity += settings.familiarity_delta;
            // Cap at the maximal familiarity.
            if self.familiarity > settings.max_familiarity {
                self.familiarity = settings.max_familiarity;
            }
        } else {
            // Decrease familiarity by the delta.
            self.familiarity -= settings.familiarity_delta;
            // Cap at the minimal familiarity.
            if self.familiarity < settings.min_familiarity {
                self.familiarity = settings.min_familiarity;
            }
        }
    }
}

/// A card schedule.
///
/// Card schedules are the persistently stored state data of Mu. They contain a number of so-called
/// "metacards", which hold learning state information of the respective card. They do not contain
/// the content of the cards.
#[derive(Serialize, Deserialize, Debug)]
pub struct Schedule {
    /// The learning states of the memory cards.
    metacards: Vec<cards::Metacard>,
    /// Statistics for all the cards.
    statistics: Statistics,
    /// Tag-specific statistics.
    tag_statistics: HashMap<String, Statistics>,
    /// When the new queue was last updated.
    ///
    /// They should update daily, so this is simply used to ensure that they will not update
    /// multiple times a day.
    updated: Option<Time>,
}

impl Schedule {
    /// Create new schedule.
    pub fn new(settings: &settings::TagSettings) -> Schedule {
        Schedule {
            metacards: Vec::new(),
            statistics: Statistics::new(settings),
            tag_statistics: HashMap::new(),
            // We start with `None`, which tells `update()` to fill new queue when called.
            updated: None,
        }
    }

    /// Load from YAML-formatted text.
    pub fn parse(input: &str) -> Result<Schedule, yaml::Error> {
        yaml::from_str(input)
    }

    /// Serialize to YAML-formatted text.
    ///
    /// This is the inverse to `parse`.
    pub fn serialize(&self) -> Result<String, yaml::Error> {
        yaml::to_string(self)
    }

    /// Get the global statistics.
    pub fn statistics(&self) -> &Statistics {
        &self.statistics
    }

    /// Get the tag-specified statistics.
    pub fn tag_statistics(&self) -> &HashMap<String, Statistics> {
        &self.tag_statistics
    }
}

// TODO: Give the scheduler a lifetime and let it use references when it can.

/// The card scheduler.
///
/// This is the main state structure of Mu. Essentially it keeps all information about cards,
/// scheduling, queues, and so on.
#[derive(Debug)]
pub struct Scheduler {
    /// The part of the scheduler that is permanently stored.
    ///
    /// It is an type invariant that all the contained metacards have existing associated cards in
    /// `deck`.
    sched: Schedule,
    /// The content of the memory cards.
    deck: deck::Deck,
    /// The current card.
    current_card: MetacardRef,
    // TODO: What about the practically impossible case where two cards get the exact same time?
    /// The queue of cards.
    ///
    /// It is assumed that either this or `Scheduler::new_queue` is nonempty.
    queue: BTreeMap<Time, MetacardRef>,
    /// New cards that have not been scheduled yet.
    new_cards: Vec<MetacardRef>,
    /// New cards that are scheduled to be reviewed.
    ///
    /// It is assumed that either this or `Scheduler::queue` is nonempty.
    new_queue: Vec<MetacardRef>,
    /// The number of cards that is due.
    ///
    /// This is supposed to reflect the number of cards in `self.queue` whose scheduled time is
    /// prior to the current point in time.
    due: usize,
}

impl Scheduler {
    /// Create a new scheduler from a deck and schedule.
    ///
    /// `deck.cards` is assumed to be nonempty.
    pub fn new(deck: deck::Deck, mut sched: Schedule) -> Scheduler {
        // The cards that have been added to the schedule or the new queue.
        let mut queued_cards = HashSet::new();

        // TODO: Change the on-disk storage structure, such that you can combine the two next two
        //       loops to a single loop over the cards in the deck.

        // Rebuild schedule and new queue.
        let mut new_cards = Vec::new();
        let mut queue = BTreeMap::new();
        for (metacard_ref, metacard) in sched.metacards.iter().enumerate() {
            // Skip cards that have been removed. Note that the data is kept, so if the card is,
            // say, uncommented, it will not have affected the state of the card.
            if !deck.cards.contains_key(&metacard.id) { continue; }

            match metacard.state {
                cards::CardState::New => {
                    new_cards.push(metacard_ref);
                },
                cards::CardState::Learning(_)
                    | cards::CardState::Relearning(_)
                    | cards::CardState::Learnt =>
                {
                    queue.insert(metacard.due, metacard_ref);
                },
            }
            // Add the card to the set of queued cards after it has been added to the scheduler.
            queued_cards.insert(metacard.id.clone());
            // TODO: Get rid of above clone. Idea: add lifetimes to Scheduler. `CardId` should be a
            //       `&str`.
        }

        // De-orphan cards that are not a part of the scheduler yet.
        for (id, card) in deck.cards.iter().filter(|(id, _)| !queued_cards.contains(id.as_str())) {
            // Add the orphaned card to the schedule.
            sched.metacards.push(cards::Metacard::new(id.clone(), deck.tag_settings(&card.tags)));
            // Add the it to the new queue.
            new_cards.push(sched.metacards.len() - 1);
        }

        // Start with empty state.
        let mut sched = Scheduler {
            sched,
            deck,
            new_cards,
            new_queue: Vec::new(),
            // Have a nonsense value, such that in case of any bugs, we should get an OOB.
            current_card: !0,
            queue,
            due: 0,
        };

        // Generate the rest of the scheduler.
        sched.update();
        // If the queues are still empty, forcibly add a new card to the new queue.
        if sched.new_queue.is_empty() && sched.queue.is_empty() {
            // We can safely unwrap, since `deck` was assumed nonempty.
            sched.new_queue.push(sched.new_cards.pop().unwrap());
        }
        // Pick a card.
        sched.pick_card();

        sched
    }

    /// Update the queues.
    fn update(&mut self) {
        // Get current time.
        let now = now();
        // Update due.
        self.due = self.queue.iter().filter(|(&time, _)| time < now).count();
        // TODO: What if self.deck.settings.max_new_queue was 0?
        // Update the card queue if it hasn't been updated today, or haven't ever been updated
        // before.
        if self.sched.updated
            .map(|updated| (updated - now).num_days() >= 1)
            .unwrap_or(true) {
            // Update time.
            self.sched.updated = Some(now);
            // Update the new cards queue. Push new cards until the daily cap or the maximal length is
            // achieved.
            for _ in self.new_queue.len()..cmp::min(
                self.deck.settings.max_new_daily + self.new_queue.len(),
                self.deck.settings.max_new_queue
            ) {
                if let Some(card) = self.new_cards.pop() {
                    self.new_queue.push(card);
                } else { break; }
            }
        }
    }

    /// Reschedule the current card.
    fn reschedule(&mut self) {
        // Get current time.
        let now = now();
        // Next due time.
        let due = self.sched.metacards[self.current_card].due;
        // Insert the card into the schedule again according to its due date.
        self.queue.insert(due, self.current_card);
        // Update due cards if necessary.
        if due <= now {
            self.due += 1;
        }
    }

    /// Pick out a new card without rescheduling the current card.
    ///
    /// `self.current_card` ought to have been rescheduled when this method is called. Otherwise,
    /// the card will be orphaned and not get into the schedule again. In other words, this is
    /// called either in the start of the scheduler or after rescheduling of the current card has
    /// been handled.
    ///
    /// This decrements `self.due` if the card is chosen from the schedule.
    fn pick_card(&mut self) {
        // Determine if the card should be picked from the new queue or from the due cards.
        let new = if self.new_queue.is_empty() {
            // There are no new cards to be introduced.
            false
        } else if self.due == 0 {
            // There are no due cards.
            true
        // Randomly choose between the new queue or the due cards. The ratio is chosen such
        // that the space between new cards is as wide as possible.
        } else if self.new_queue.len() <= self.due {
            rand::thread_rng().gen_ratio(self.new_queue.len() as u32, self.due as u32)
        } else {
            !rand::thread_rng().gen_ratio(self.due as u32, self.new_queue.len() as u32)
        };

        if new {
            // TODO: It is likely better to pop from the other end such that the earliest
            //       introduced cards gets into play even if the new queue piles up.
            // Simply pop from the new queue (possibly yielding `None`). We can safely unwrap here,
            // since `new` is false when `self.new_queue` is empty.
            self.current_card = self.new_queue.pop().unwrap();
        } else {
            // Take out the next card from the queue. We can safely unwrap, since it is assumed
            // that `self.queue` is nonempty.
            let (&time, &card) = self.queue.iter().next().unwrap();
            self.queue.remove(&time);
            // Update.
            self.current_card = card;
            if self.due != 0 {
                self.due -= 1;
            }
        }
    }

    /// Update the scheduler after a card was reviewed.
    pub fn review(&mut self, score: cards::Score) {
        // TODO: Ideally you would do the following, but borrowck fails:
        // let card_id = &self.sched.metacards[self.current_card].id;

        // Update the card.
        let tag_settings = self.deck.tag_settings(&self.deck.cards[&self.sched.metacards[self.current_card].id].tags);

        // The sum of the familiarities. Start with the global familiarity.
        let mut familiarity_sum = self.sched.statistics.familiarity;
        // The number of tags with familiarity statistics.
        let mut familiarity_num = 1;

        // Update statistics.
        self.sched.statistics.review(score, &tag_settings);
        // Update tagwise statistics.
        let tags = &self.deck.cards[&self.sched.metacards[self.current_card].id].tags;
        for tag in tags {
            if let Some(stat) = self.sched.tag_statistics.get_mut(tag) {
                // Update the sum.
                familiarity_sum += stat.familiarity;
                familiarity_num += 1;
                // Register the review.
                stat.review(score, &tag_settings);
            } else {
                // Create a new statistics tracker for the tag if it does not already exist.
                let mut stat = Statistics::new(&tag_settings);
                stat.review(score, &tag_settings);
                self.sched.tag_statistics.insert(tag.clone(), stat);
            }
        }

        // Calculate the average of the familiarity.
        let average_familiarity = familiarity_sum / familiarity_num as f32;
        // Register the review on the metacard.
        let priority = self.deck.cards[&self.sched.metacards[self.current_card].id].priority;
        self.sched.metacards[self.current_card].review(tag_settings, score, priority, average_familiarity);

        // Add back the card to the schedule.
        self.reschedule();

        // Pick a new card.
        self.pick_card();
        // Update the scheduler.
        self.update();
    }

    /// Get the current card's metacard.
    pub fn current_metacard(&self) -> &cards::Metacard {
        &self.sched.metacards[self.current_card]
    }

    /// Get the current card.
    pub fn current_card(&self) -> &cards::Card {
        // Look up the card. Note that this never panics, since it is an invariant of the
        // `Scheduler` that all the metacards have existent cards.
        &self.deck.cards[&self.current_metacard().id]
    }

    /// Get the possible new intervals after review, ordered after score.
    ///
    /// These possible intervals are supposed to be shown to the user after they have reviewed all
    /// sides of the card, before they provide the score for the review.
    pub fn current_card_new_intervals(&self) -> [chrono::Duration; cards::SCORES] {
        // Get current card.
        let card = self.current_card();
        // TODO: This code is effectively the same as the calculation of average in `review`.
        //       Perhaps move it to a distinct method of some kind.
        // The sum of the familiarities. Start with the global familiarity.
        let mut familiarity_sum = self.sched.statistics.familiarity;
        // The number of tags with familiarity statistics.
        let mut familiarity_num = 1;
        // Calculate the sum, in order to obtain average.
        for tag in &card.tags {
            if let Some(stat) = self.sched.tag_statistics.get(tag) {
                familiarity_sum += stat.familiarity;
                familiarity_num += 1;
            }
        }

        // Calculate the new intervals.
        self.current_metacard().new_intervals(self.deck.tag_settings(&card.tags), card.priority, familiarity_sum / familiarity_num as f32)
    }

    /// Get the number of due cards.
    pub fn due_cards(&self) -> usize {
        self.due
    }

    /// Get the number of cards to be reviewed (both including due and new queue).
    pub fn queued_cards(&self) -> usize {
        self.due + self.new_queue.len()
    }

    /// Get the number of new cards.
    pub fn new_cards(&self) -> usize {
        self.new_queue.len()
    }

    /// Get the underlying schedule.
    pub fn schedule(&self) -> &Schedule {
        &self.sched
    }
}

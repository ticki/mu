//! Collections of flashcards.

use std::collections::HashMap;
use std::{num, mem, fmt, error};

use chrono;

use crate::cards;
use crate::settings;

/// A collection of flashcards, without any data about learning state.
#[derive(Debug)]
pub struct Deck {
    /// The global settings.
    pub settings: settings::GlobalSettings,
    /// The setting for the various tags.
    ///
    /// The first tag that has associated settings will apply to the card. If none of the tags have
    /// associated settings, it will apply the settings from the tag `""`.
    pub tag_settings: HashMap<String, settings::TagSettings>,
    /// The content of the cards.
    pub cards: HashMap<cards::CardId, cards::Card>,
}

// TODO: Instead of `ParsingError::Other`, have a bunch of smaller variants.

impl Deck {
    /// Parse deck from `.mu` format.
    ///
    /// This will give an error if the deck was empty.
    pub fn parse(src: &str) -> Result<Deck, ParsingErrorLine> {
        // Parse.
        let mut parser = Parser::default();
        parser.parse(src)?;
        // Ensure that the deck is nonempty.
        if parser.deck.cards.is_empty() {
            Err(ParsingErrorLine {
                err: ParsingError::Other("empty deck"),
                line_num: 0,
            })
        } else { Ok(parser.deck) }
    }

    /// Clone the default tag settings.
    ///
    /// All tag setting groups inherit their default values fromm the default settings, which are
    /// specified under section `[tag default]`.
    fn default_tag_settings(&self) -> settings::TagSettings {
        self.tag_settings[""].clone()
    }

    /// Get the relevant settings for a card with tags `tags`.
    pub fn tag_settings(&self, tags: &[String]) -> &settings::TagSettings {
        // Search for the tag settings.
        for tag in tags {
            if let Some(settings) = self.tag_settings.get(tag) {
                return settings;
            }
        }

        // If no tags had associated settings, use the `""` tag.
        &self.tag_settings[""]
    }
}

impl Default for Deck {
    fn default() -> Deck {
        Deck {
            settings: settings::GlobalSettings::default(),
            tag_settings: {
                let mut hm = HashMap::new();
                hm.insert(String::new(), settings::TagSettings::default());
                hm
            },
            cards: HashMap::new(),
        }
    }
}

/// Parse a key-value pair in the format `<key>: <value>`.
///
/// The colon may be surronded by whitespaces, which will be trimmed. However, the start and end of
/// `s` is not trimmed.
fn key_value(s: &str) -> Result<(&str, &str), ParsingError> {
    let colon = s.find(':').ok_or(ParsingError::Other("not a proper key-value pair (no colon present)"))?;
    // Check out-of-bounds to avoid panic.
    if colon + 1 < s.len() {
        Ok((s[..colon].trim_end(), s[colon + 1..].trim_start()))
    } else {
        Err(ParsingError::Other("no value specified"))
    }
}

/// Parse duration.
///
/// This parses a string of form `<integer><unit>` where `<unit>` is one of the following:
///
/// - `m` for minute.
/// - `d` for day.
/// - `w` for week.
/// - `M` for month.
/// - `y` for year.
fn parse_duration(s: &str) -> Result<chrono::Duration, ParsingError> {
    // Find the length of the number part of `s` and the unit (the last character of `s`).
    let (number_part, unit) = s.char_indices().last().ok_or(ParsingError::Other("duration empty"))?;
    // Parse the number part.
    let number = s[..number_part].parse()?;

    // Convert into `chrono::Duration` accordingly.
    match unit {
        'm' => Ok(chrono::Duration::minutes(number)),
        'd' => Ok(chrono::Duration::days(number)),
        'w' => Ok(chrono::Duration::weeks(number)),
        'M' => Ok(chrono::Duration::weeks(number * 4)),
        'y' => Ok(chrono::Duration::weeks(number * 4 * 12)),
        // TODO: Better error message. Output `unit`.
        _   => Err(ParsingError::Other("unknown unit")),
    }
}

/// Parse comma-separated list.
///
/// `parser` parses the individual items.
fn parse_list<F, T, E>(s: &str, parser: F) -> Result<Vec<T>, ParsingError>
    where F: Fn(&str) -> Result<T, E>,
          ParsingError: From<E>
{
    s
        // Split at commas.
        .split(',')
        // Trim each item.
        .map(str::trim)
        // Parse the items.
        .map(parser)
        // Collect into vector with (somewhat awkward) error handling.
        .try_fold(Vec::new(), |mut acc, item| {
            acc.push(item?);
            Ok(acc)
        })
}

/// Convert vector to array with entries corresponding to `cards::Score`s.
///
/// This is used for settings that takes an entry for each score. It check that the vector is the
/// correct length (`cards::SCORES`) and converts it into an array.
fn to_score_array<T: Default + Copy>(vec: Vec<T>)
    -> Result<[T; cards::SCORES], ParsingError>
{
    // Ensure that `vec` has the correct length.
    if vec.len() == cards::SCORES {
        // This is optimized away; no need for `unsafe`.
        let mut arr: [T; cards::SCORES] = Default::default();
        // Copy each item over.
        for i in 0..cards::SCORES {
            arr[i] = vec[i];
        }

        Ok(arr)
    } else {
        Err(ParsingError::Other("wrong number of items in the list (expected the number of scores)"))
    }
}

/// The deck parser's state.
enum ParserState {
    /// Currently parsing the global settings.
    ///
    /// The global settings are simply a list of key-value pairs.
    GlobalSettings,
    /// Currently parsing tag-specific settings.
    ///
    /// These settings are simply a list of key-value pairs.
    TagSettings(String),
    /// Currently parsing a card.
    ///
    /// It enters this state after a section whose title is a card ID.
    Card(cards::CardId),
    /// The parser has just been flushed and waits for a new state.
    Flushed,
}

impl Default for ParserState {
    fn default() -> ParserState {
        ParserState::Flushed
    }
}

// TODO: Should errors start on uppercase?

/// An error during parsing.
#[derive(Debug)]
pub enum ParsingError {
    /// Error during integer parsing.
    ParseInt(num::ParseIntError),
    /// Error during float parsing.
    ParseFloat(num::ParseFloatError),
    /// Other error.
    Other(&'static str),
}

impl From<num::ParseIntError> for ParsingError {
    fn from(error: num::ParseIntError) -> Self {
        ParsingError::ParseInt(error)
    }
}

impl From<num::ParseFloatError> for ParsingError {
    fn from(error: num::ParseFloatError) -> Self {
        ParsingError::ParseFloat(error)
    }
}

/// A parsing error with an associated line number.
#[derive(Debug)]
pub struct ParsingErrorLine {
    /// The line number.
    line_num: usize,
    /// The error.
    err: ParsingError,
}

impl error::Error for ParsingErrorLine {
    fn description(&self) -> &str {
        "parsing error"
    }

    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self.err {
            ParsingError::ParseInt(ref err) => Some(err),
            ParsingError::ParseFloat(ref err) => Some(err),
            ParsingError::Other(..) => None,
        }
    }
}

impl fmt::Display for ParsingErrorLine {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.err {
            ParsingError::ParseInt(ref err) => write!(f, "failed to parse integer ({}); at line {}", err, self.line_num),
            ParsingError::ParseFloat(ref err) => write!(f, "failed to parse float ({}); at line {}", err, self.line_num),
            ParsingError::Other(ref err) => write!(f, "{}; at line {}", err, self.line_num),
        }
    }
}

/// A `.mu`-file parser.
#[derive(Default)]
struct Parser {
    /// The deck.
    deck: Deck,
    /// The current state of the parser.
    state: ParserState,
    /// Current tag-specific settings.
    ///
    /// This is built incrementally as more lines are parsed. To manifest it to `deck`, use
    /// `flush()`. Note that it is only relevant when `state` is `ParsingState::TagSettings`.
    current_tag_settings: settings::TagSettings,
    /// Current card.
    ///
    /// This is built incrementally as more lines are parsed. To manifest it to `deck`, use
    /// `flush()`. Note that it is only relevant when `state` is `ParsingState::CardMeta` or
    /// `ParsingState::CardSides`.
    current_card: cards::Card,
}

impl Parser {
    /// Parse `src` and update state accordingly.
    fn parse<'a>(&mut self, src: &'a str) -> Result<(), ParsingErrorLine> {
        // The current line number.
        let mut line_num = 0;
        // Parse line-by-line.
        for line in src.lines() {
            self.parse_line(line).map_err(|err| ParsingErrorLine { err, line_num })?;
            line_num += 1;
        }

        // Flush the last state.
        self.flush().map_err(|err| ParsingErrorLine { err, line_num })
    }

    /// Flush changes.
    ///
    /// This ought to be called after sections have been completed and in the end of the file.
    fn flush(&mut self) -> Result<(), ParsingError> {
        // Replace the old state with the `Flushed` state.
        match mem::replace(&mut self.state, ParserState::Flushed) {
            // The global settings are written directly to the deck; nothing to flush.
            ParserState::GlobalSettings => (),
            // TODO: Get rid of the hack `tag != ""` and detect overwritten settings for the
            //       default tag as well. It is important to use `&` and not `&&` to avoid
            //       short-circuiting.
            // Insert the new tag settings.
            ParserState::TagSettings(tag) => if (tag != "") & self.deck.tag_settings
                // Swap the current settings with the default settings.
                .insert(tag, mem::replace(&mut self.current_tag_settings, self.deck.default_tag_settings()))
                .is_some() {
                    // Throw an error if the settings already exist in the deck.
                    return Err(ParsingError::Other("configuring a tag multiple times (previous section)"));
                },
            // Insert the card.
            ParserState::Card(id) => if self.deck.cards
                // Swap the current card with a default, empty card.
                // TODO: Get rid of this clone.
                .insert(id.clone(), {
                    let mut card = mem::replace(&mut self.current_card, Default::default());
                    // If no files were specified for the card, default to its ID with a `.pdf`
                    // extension.
                    if card.view.is_empty() {
                        // Add the path to the list of files.
                        card.view.push(cards::View::Pdf(format!("{}.pdf", id)));
                    }

                    card
                })
                .is_some() {
                    // Throw an error if the card already exists in the deck.
                    return Err(ParsingError::Other("the same card ID appears multiple times"));
                },
            // Already flushed; do nothing.
            ParserState::Flushed => (),
        }

        Ok(())
    }

    /// Parse a single line and update state accordingly.
    fn parse_line(&mut self, mut line: &str) -> Result<(), ParsingError> {
        // Canonicalize lines.
        line = line.trim();
        if line.is_empty() {
            return Ok(());
        }

        // Skip comments.
        if line.starts_with('#') {
            return Ok(());
        // Handle new section.
        } else if line.starts_with('[') && line.ends_with(']') {
            // Close off existing section.
            self.flush()?;

            // Update the state.
            self.state = match line[1..line.len() - 1].trim() {
                // Global settings.
                "settings" => ParserState::GlobalSettings,
                // Default tag settings.
                "tag default" => ParserState::TagSettings(String::new()),
                // Tag specific settings.
                title if title.starts_with("tag ")
                    => ParserState::TagSettings(
                        // Take the part representing the tag name, following `tag ` in the section
                        // name.
                        title["tag ".len()..].trim_start().to_string()
                    ),
                // Otherwise, it must be a card.
                title if title.starts_with("card ")
                    => ParserState::Card(
                        // Take the part representing the card ID, following `card ` in the section
                        // name.
                        title["card ".len()..].trim_start().to_string()
                    ),
                _ => return Err(ParsingError::Other("unknown section")),
            };

            return Ok(());
        }

        match self.state {
            ParserState::GlobalSettings => {
                // Read key-value pair.
                let (key, value) = key_value(line)?;

                // Update global settings.
                match key {
                    "max new queue" => self.deck.settings.max_new_queue = value.parse()?,
                    "max new daily" => self.deck.settings.max_new_daily = value.parse()?,
                    _ => return Err(ParsingError::Other("unknown key")),
                }
            },
            ParserState::TagSettings(..) => {
                // Read key-value pair.
                let (key, value) = key_value(line)?;

                // Update tag settings.
                match key {
                    "INHERIT" => self.current_tag_settings = self.deck.tag_settings
                        .get(value)
                        .ok_or(ParsingError::Other("cannot inherit nonexistent settings"))?
                        .clone(),
                    "learning intervals"
                        => self.current_tag_settings.learning_intervals = parse_list(value, parse_duration)?,
                    "learning interval progressions"
                        => self.current_tag_settings.learning_interval_progressions = to_score_array(parse_list(value, str::parse)?)?,
                    "relearning intervals"
                        => self.current_tag_settings.relearning_intervals = parse_list(value, parse_duration)?,
                    "relearning interval progressions"
                        => self.current_tag_settings.relearning_interval_progressions = to_score_array(parse_list(value, str::parse)?)?,
                    "max interval" => self.current_tag_settings.max_interval = parse_duration(value)?,
                    "min interval increase"
                        => self.current_tag_settings.min_interval_increase = parse_duration(value)?,
                    "starting ease" => self.current_tag_settings.starting_ease = value.parse()?,
                    "min ease" => self.current_tag_settings.min_ease = value.parse()?,
                    "max ease" => self.current_tag_settings.max_ease = value.parse()?,
                    "ease increase"
                        => self.current_tag_settings.ease_increase = to_score_array(parse_list(value, str::parse)?)?,
                    "interval modifier" => self.current_tag_settings.interval_modifier = value.parse()?,
                    "score modifiers"
                        => self.current_tag_settings.score_modifiers = to_score_array(parse_list(value, str::parse)?)?,
                    "priority modifiers"
                        => self.current_tag_settings.priority_modifiers = to_score_array(parse_list(value, str::parse)?)?,
                    "score weight"
                        => self.current_tag_settings.score_weight = value.parse()?,
                    "familiarity delta"
                        => self.current_tag_settings.familiarity_delta = value.parse()?,
                    "max familiarity"
                        => self.current_tag_settings.max_familiarity = value.parse()?,
                    "min familiarity"
                        => self.current_tag_settings.min_familiarity = value.parse()?,
                    "desired retention rate"
                        => self.current_tag_settings.desired_retention_rate = value.parse()?,
                    _ => return Err(ParsingError::Other("unknown key")),
                }
            },
            ParserState::Card(..) => {
                // Read key-value pair.
                let (key, value) = key_value(line)?;

                // Update current card.
                match key {
                    "pdf" => self.current_card.view.extend(value.split(',').map(|x| x.trim().into()).map(cards::View::Pdf)),
                    "sh" => self.current_card.view.push(cards::View::Command(cards::Command(value.to_string()))),
                    "tags" => self.current_card.tags.extend(value.split(',').map(|x| x.trim().to_string())),
                    "max interval" => self.current_card.max_interval = parse_duration(value)?,
                    "priority" => {
                        // Parse the priority
                        let priority = value.parse::<cards::Priority>()? - 1;
                        // Handle invalid priorities.
                        if priority > 4 {
                            return Err(ParsingError::Other("invalid priority value; must be 1-5"));
                        }
                        // Update priority.
                        self.current_card.priority = priority;
                    },
                    _ => return Err(ParsingError::Other("unknown key")),
                }
            },
            // TODO: Somehow, this is not unreachable. Try to parse a random file.
            ParserState::Flushed => unreachable!(),
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse() {
        let input = r"

[settings]
    max new queue: 20
    max new daily: 5

[tag default]
    learning intervals: 30m, 1d, 3d
    learning interval progressions: -2, 1, 1, 2, 2
    relearning intervals: 30m, 1d
    relearning interval progressions: -2, 1, 1, 1, 1
    starting ease: 2.5
ease increase: -0.20, -0.15, 0, 0.05, 0.15
    interval modifier: 1
    score modifiers: 1, 0.7, 1, 1.2, 1.4
    priority modifiers: 1.5, 1.2, 1, 0.8, 0.5
    max interval: 2M
    desired retention rate: 0.85

[tag Definition]
    learning intervals: 30m, 1d, 3d
    learning interval progressions: -2, 1, 1, 2, 2
    relearning intervals: 30m, 1d
    relearning interval progressions: -2, 1, 1, 1, 1
    starting ease: 2.5
    ease increase: -0.20, -0.15, 0, 0.05, 0.15
    interval modifier: 1
    score modifiers: 1, 0.7, 1, 1.2, 1.4
    priority modifiers: 1.5, 1.2, 1, 0.8, 0.5
    max interval: 4y
    desired retention rate: 0.85

[tag Theorem]
    INHERIT: Definition

[tag Exercise]
# Comment here
    ease increase: -0.20, -0.15, 0, 0.05, 0.15
    interval modifier: 1
    score modifiers: 1, 0.7, 1, 1.2, 1.4
    desired retention rate: 0.85

[card 123]
tags: Definition, Week 2
priority: 5
";
        let deck = Deck::parse(input).unwrap();
        assert_eq!(
            chrono::Duration::weeks(4 * 12 * 4),
            deck.tag_settings["Theorem"].max_interval,
        );
        assert_eq!(deck.cards["123"].tags[0], "Definition");
        assert_eq!(deck.cards["123"].priority, 4);
    }

    #[test]
    fn no_settings() {
        Deck::parse("
[card 123]
tags: Definition, Week 2
priority: 5
file: fibration.pdf



[card 124]
tags: Definition
priority: 5
file: derived_couple.pdf

[card 125]
tags: Definition
priority: 5
file: triad.pdf
max interval: 500d

[card 127]
tags: Definition
priority: 5
file: spectral_sequence.pdf
        ").unwrap();
    }

    #[test]
    #[should_panic]
    fn error_double_section1() {
        Deck::parse("

[tag A]
[tag A]
        ").unwrap();
    }

    #[test]
    #[should_panic]
    fn error_double_section2() {
        Deck::parse("
[card a123]
tags: Definition, Week 2
priority: 5
file: module.pdf



[card a123]
tags: Definition
priority: 5
file: ring.pdf
        ").unwrap();
    }

    #[test]
    #[should_panic]
    fn error_unknown_key1() {
        Deck::parse("

[settings]
    max new queue: 20
    max new daily: 5
    min new daily: 5
        ").unwrap();
    }

    #[test]
    #[should_panic]
    fn error_unknown_key2() {
        Deck::parse(r"
[card afhe]
tags: Definition, Week 2
priorityy: 5
file: module.pdf



[card afhd]
tags: Definition
priority: 5
file: module.pdf
        ").unwrap();
    }

    #[test]
    #[should_panic]
    fn error_unknown_units() {
        Deck::parse(r"

[settings]
    max new queue: 20
    max new daily: 5

[tag default]
    learning intervals: 30m, 1a, 3d
    learning interval progressions: -2, 1, 1, 2, 2
            relearning intervals: 30m, 1d
            relearning interval progressions: -2, 1, 1, 1, 1
            starting ease: 2.5
        ease increase: -0.20, -0.15, 0, 0.05, 0.15
            interval modifier: 1
            score modifiers: 1, 0.7, 1, 1.2, 1.4
            priority modifiers: 1.5, 1.2, 1, 0.8, 0.5
            max interval: 2M
            desired retention rate: 0.85
        ").unwrap();
    }
}

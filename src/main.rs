extern crate termion;
extern crate mu_backend as backend;
extern crate clap;
extern crate failure;
extern crate chrono;
extern crate itertools;

// TODO: Add help page for formatting of .mu files.

use std::io::{self, Read, Write};
use std::{path, fs, fmt, env, process};

use failure::Error;
use termion::{color, style};
use itertools::Itertools;
use clap::{Arg, App};

/// The text that is printed when the `help` command is issued.
const HELP: &'static str = r#"view, v  : View the current card
info, i  : Print card info
meta, m  : Print card meta data
hist, hi : Print card history
tags, tg : Print tag statistics
help, he : Print this help page
quit, q  : Quit the program
fail, f  : Review the card as failed
hard, h  : Review the card as hard
okay, o  : Review the card as okay
good, g  : Review the card as good
easy, e  : Review the card as easy
post, p  : Postpone the card to tomorrow"#;

/// Formatter for durations.
struct DurationFormatter(chrono::Duration);

impl fmt::Display for DurationFormatter {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // Break the duration into units.
        let mut duration = self.0;

        if duration.is_zero() {
            // If the duration is empty, simply write an unitless 0.
            write!(f, "0")
        } else {
            // Calculate the various units.
            let years = duration.num_weeks() / 4 / 12;
            // TODO: `-=` is not implemented upstream yet. Change the code when it is.
            duration = duration - chrono::Duration::weeks(years * 4 * 12);
            let months = duration.num_weeks() / 4;
            duration = duration - chrono::Duration::weeks(months * 4);
            let weeks = duration.num_weeks();
            duration = duration - chrono::Duration::weeks(weeks);
            let days = duration.num_days();
            duration = duration - chrono::Duration::days(days);
            let hours = duration.num_hours();
            duration = duration - chrono::Duration::hours(hours);
            let minutes = duration.num_minutes();

            // Write non-zero parts.
            if years != 0 { write!(f, "{}y ", years)?; }
            if months != 0 { write!(f, "{}M ", months)?; }
            if weeks != 0 { write!(f, "{}w ", weeks)?; }
            if days != 0 { write!(f, "{}d ", days)?; }
            if hours != 0 { write!(f, "{}h ", hours)?; }
            if minutes != 0 { write!(f, "{}m ", minutes)?; }

            Ok(())
        }
    }
}

/// State of application in "review" mode.
pub struct State<W: Write, R> {
    /// The card scheduler.
    scheduler: backend::Scheduler,
    /// Path to the schedule file.
    schedule_path: path::PathBuf,
    /// Standard output.
    stdout: W,
    /// Standard input.
    stdin: io::Lines<R>,
}

impl<W: Write, R: io::BufRead> State<W, R> {
    /// Create a new `State`.
    pub fn new(stdout: W, stdin: R, deck_path: &path::Path, schedule_path: path::PathBuf) -> Result<State<W, R>, Error> {
        let (scheduler, schedule_path) = load(deck_path, schedule_path)?;

        Ok(State {
            scheduler,
            schedule_path,
            stdout: stdout,
            stdin: stdin.lines(),
        })
    }

    /// Write the state to the schedule file.
    fn write(&mut self) -> Result<(), Error> {
        // Serialize the schedule.
        let data = self.scheduler.schedule().serialize()?;
        // Write it to the file.
        fs::File::create(&self.schedule_path)?.write(data.as_bytes())?;
        Ok(())
    }

    /// Run the program.
    pub fn run(mut self) -> Result<(), Error> {
        // Show new card.
        self.show_card()?;
        // Print the shell.
        self.print_shell()?;

        // TODO: Perhaps there is a better syntax for iterating.
        while let Some(line) = self.stdin.next() {
            // Read command.
            if !self.command(&line?)? { break; };
            // Print new shell.
            self.print_shell()?;
        }

        Ok(())
    }

    /// Print the shell, that is, the text before the command input.
    pub fn print_shell(&mut self) -> Result<(), Error> {
        write!(self.stdout, "D:{} N:{} {}>>{} ",
            self.scheduler.due_cards(),
            self.scheduler.new_cards(),
            color::Fg(color::Red),
            color::Fg(color::Reset),
        )?;
        // Print it immediately.
        self.stdout.flush()?;
        Ok(())
    }

    /// Run command `command`.
    ///
    /// The returned boolean is false precisely when the program should quit.
    fn command(&mut self, mut command: &str) -> Result<bool, Error> {
        // Remove any whitespaces in either ends.
        command = command.trim();
        // Do the respective action.
        match command {
            // View a card.
            "view" | "v" => self.view_card()?,
            // Review: fail.
            "fail" | "f" => self.review(backend::Score::Fail)?,
            // Review: hard.
            "hard" | "h" => self.review(backend::Score::Hard)?,
            // Review: okay.
            "okay" | "o" => self.review(backend::Score::Okay)?,
            // Review: good.
            "good" | "g" => self.review(backend::Score::Good)?,
            // Review: easy.
            "easy" | "e" => self.review(backend::Score::Easy)?,
            // Postpone the card.
            "post" | "p" => self.postpone()?,
            // Print card information.
            "info" | "i" => self.print_info()?,
            // Print metadata of the card.
            "meta" | "m" => self.print_meta()?,
            // Print history of the card.
            "hist" | "hi" => self.print_history()?,
            // Print tag statistics.
            "tags" | "tg" => self.print_tag_statistics()?,
            // Quit the program.
            "quit" | "q" => return Ok(false),
            // Print help screen.
            "help" | "he" => self.help()?,
            // Skip.
            "" => (),
            // Unknown command.
            _ => writeln!(self.stdout, "Unknown command '{}'.", command)?,
        }

        // The program will continue.
        Ok(true)
    }

    /// Print a section header.
    ///
    /// This is used to mark the various parts in the output.
    fn print_header(&mut self, f: fmt::Arguments) -> Result<(), Error> {
        writeln!(self.stdout, "{}——— {} ———{}", style::Bold, f, style::Reset)?;
        Ok(())
    }

    /// Print help screen.
    fn help(&mut self) -> Result<(), Error> {
        self.print_header(format_args!("help"))?;
        writeln!(self.stdout, "{}", HELP)?;
        Ok(())
    }

    /// Review the current card with score `score`.
    fn review(&mut self, score: backend::Score) -> Result<(), Error> {
        // Review the card.
        self.scheduler.review(score);
        // Write the schedule to the file system.
        self.write()?;
        // Show the new card.
        self.show_card()?;
        Ok(())
    }

    /// Postpone the card to tomorrow.
    fn postpone(&mut self) -> Result<(), Error> {
        // Postpone the card.
        self.scheduler.postpone();
        // Write the schedule to the file system.
        self.write()?;
        // Write a message to te user.
        writeln!(self.stdout, "card is now due in 24 hours.")?;
        // Show the new card.
        self.show_card()?;
        Ok(())
    }

    /// Show new card.
    ///
    /// This opens a viewer and prints necessary information.
    fn show_card(&mut self) -> Result<(), Error> {
        // Print card information.
        self.print_info()?;
        // View the card.
        self.view_card()?;
        // Print the new intervals.
        self.print_intervals()
    }

    /// Print card information.
    fn print_info(&mut self) -> Result<(), Error> {
        // Print the header.
        // TODO: Get rid of this unnecessary clone simply there to please borrowck.
        let id = &self.scheduler.current_metacard().id.clone();
        self.print_header(format_args!("card '{}'", id))?;
        // Print the card data.
        writeln!(self.stdout, "file:      {}", self.scheduler
            .current_card()
            .view
            .iter()
            .map(|file| file.as_str())
            .format(", ")
        )?;
        writeln!(self.stdout, "tags:      {}", self.scheduler.current_card().tags.iter().format(", "))?;
        writeln!(self.stdout, "priority:  {}", self.scheduler.current_card().priority)?;

        Ok(())
    }

    /// Print card metadata.
    fn print_meta(&mut self) -> Result<(), Error> {
        // Print header.
        self.print_header(format_args!("meta"))?;
        // Write the metadata.
        let meta = self.scheduler.current_metacard();
        writeln!(self.stdout, "id:        {}", meta.id)?;
        writeln!(self.stdout, "state:     {:?}", meta.state)?;
        writeln!(self.stdout, "ease:      {}", meta.ease)?;
        writeln!(self.stdout, "interval:  {}", DurationFormatter(meta.current_interval))?;
        writeln!(self.stdout, "due:       {}", meta.due.to_rfc2822())?;

        Ok(())
    }

    /// Print history of the card.
    fn print_history(&mut self) -> Result<(), Error> {
        // Print header.
        self.print_header(format_args!("history"))?;
        // Print all the reviews.
        for (n, review) in self.scheduler.current_metacard().history.iter().enumerate() {
            writeln!(self.stdout, "review {}:", n)?;
            writeln!(self.stdout, "    time:            {}", review.time.to_rfc2822())?;
            writeln!(self.stdout, "    due:             {}", review.due.to_rfc2822())?;
            writeln!(self.stdout, "    score:           {}", review.score)?;
            writeln!(self.stdout, "    ended interval:  {}", DurationFormatter(review.ended_interval))?;
            writeln!(self.stdout, "    state before:    {:?}", review.state_before)?;
            writeln!(self.stdout, "    ease before:     {:?}", review.ease_before)?;
        }

        Ok(())
    }

    /// Print next intervals of the card.
    fn print_intervals(&mut self) -> Result<(), Error> {
        // Calculate new intervals.
        let intervals = self.scheduler.current_card_new_intervals();
        // Print header.
        self.print_header(format_args!("new intervals"))?;
        // Print intervals.
        writeln!(self.stdout, "fail:  {}", DurationFormatter(intervals[0]))?;
        writeln!(self.stdout, "hard:  {}", DurationFormatter(intervals[1]))?;
        writeln!(self.stdout, "okay:  {}", DurationFormatter(intervals[2]))?;
        writeln!(self.stdout, "good:  {}", DurationFormatter(intervals[3]))?;
        writeln!(self.stdout, "easy:  {}", DurationFormatter(intervals[4]))?;

        Ok(())
    }

    /// Print the tag statistics.
    fn print_tag_statistics(&mut self) -> Result<(), Error> {
        // Print header.
        self.print_header(format_args!("tag statistics"))?;
        // Print retention rate for all cards.
        let sched = self.scheduler.schedule();
        writeln!(self.stdout, "NAME: RETENTION%, FAMILIARITY")?;
        writeln!(self.stdout, "{}all{}: {:.1}%, {:.2}",
            style::Underline,
            style::Reset,
            sched.statistics().retention_rate() * 100.0,
            sched.statistics().familiarity(),
        )?;
        // Print retention rates for each tag.
        for (tag, stat) in sched.tag_statistics() {
            writeln!(self.stdout, "{}: {:.1}%, {:.2}",
                tag,
                stat.retention_rate() * 100.0,
                stat.familiarity(),
            )?;
        }

        Ok(())
    }

    /// View the current card.
    fn view_card(&mut self) -> Result<(), Error> {
        // Get the current card.
        let card = self.scheduler.current_card();
        // View each of its associated cards.
        for view in &card.view {
            match view {
                backend::View::Pdf(path) => {
                    writeln!(self.stdout, "(opening {})", path)?;
                    // TODO: Make PDF reader customizable.
                    // Start program for viewing.
                    process::Command::new("/bin/sh")
                        .arg("-c")
                        .arg(match env::var("MU_PDF_VIEWER") {
                            Ok(ref pdf_viewer) if !pdf_viewer.is_empty() => format!("{} '{}'", pdf_viewer.as_str(), path),
                            _ => format!("zathura --mode=presentation --page=0 '{}'", path),
                        })
                        .stdin(process::Stdio::piped())
                        .spawn()?
                        .wait()?;
                },
                backend::View::Command(cmd) => {
                    cmd.execute()?.wait()?;
                },
            }
        }

        Ok(())
    }
}

/// Load a deck and schedule.
///
/// This returns a scheduler and a path to the schedule (a canonicalized version of
/// `schedule_path`).
fn load(deck_path: &path::Path, mut schedule_path: path::PathBuf) -> Result<(backend::Scheduler, path::PathBuf), Error> {
    // Open files.
    let mut deck_file = fs::File::open(deck_path)?;
    let mut schedule_file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(&schedule_path)?;
    // Get the absolute path to the schedule path before changing directory.
    schedule_path = fs::canonicalize(schedule_path)?;
    // Change the directory to the directory of the schedule file.
    let deck_path = fs::canonicalize(deck_path)?;
    env::set_current_dir(deck_path.parent().unwrap())?;
    // Read files.
    let mut deck_buffer = String::new();
    deck_file.read_to_string(&mut deck_buffer)?;
    let mut schedule_buffer = String::new();
    schedule_file.read_to_string(&mut schedule_buffer)?;
    // Parse.
    let deck = backend::Deck::parse(&deck_buffer)?;
    let schedule = if schedule_buffer.is_empty() {
        // When the file is empty (e.g. first time the schedule is loaded), use the default,
        // empty schedule.
        backend::Schedule::new(&deck.tag_settings[""])
    } else {
        backend::Schedule::parse(&schedule_buffer)?
    };

    Ok((backend::Scheduler::new(deck, schedule), schedule_path))
}

// TODO: Better error messages
/// Start mu.
fn main_err() -> Result<(), Error> {
    // Parse flags etc..
    let matches = App::new("Mu")
        .version("0.1.0")
        .about("Advanced Unix-style Spaced Repetition System")
        .arg(Arg::with_name("DECK")
             .help("Sets the '.mu' deck file to use")
             .default_value("deck.mu"))
        .arg(Arg::with_name("schedule")
             .short("s")
             .long("schedule")
             .value_name("FILE")
             // TODO: Use a more automated way of specifying the default value.
             .help("Sets an alternative '.mu.sched' schedule file [default: <DECK>.sched]")
             .takes_value(true))
        .arg(Arg::with_name("queued")
             .short("q")
             .long("queued")
             // TODO: Use a more automated way of specifying the default value.
             .help("Prints number of cards to be reviewed and quits"))
        .get_matches();

    // Lock stdout.
    let stdout = io::stdout();
    let mut stdout = stdout.lock();

    // The deck of cards.
    let deck = path::PathBuf::from(matches.value_of("DECK").unwrap());

    // When `--schedule` is not specified, this will hold the `PathBuf` of the schedule file.
    // (Premature optimization.)
    let schedfile;
    // Obtain the schedule file.
    let schedule = if let Some(path) = matches.value_of("schedule") {
        // A custom schedule file was specified by the user.
        path::PathBuf::from(path)
    } else {
        // No custom schedule file was specified. Revert to default behavior: Add the extension
        // `.sched` to the deck file.
        // TODO: This is not quite in line with what the help information says. Indeed, it is
        //       supposed to extend by `.sched` not change the extension.
        schedfile = deck.with_extension("mu.sched");
        schedfile
    };

    if matches.occurrences_of("queued") == 0 {
        // Run in normal mode.

        // Initialize stdout and stdin.
        let stdin = io::stdin();

        // Run the program.
        State::new(stdout, stdin.lock(), &deck, schedule)?.run()?;
    } else {
        // Print number of to-do cards.
        writeln!(stdout, "{}", load(&deck, schedule)?.0.queued_cards())?;
    }

    Ok(())
}

fn main() -> Result<(), Error> {
    // Run Mu.
    if let Err(err) = main_err() {
        // Handle errors.
        eprintln!("Error: {}", err);
    }

    Ok(())
}

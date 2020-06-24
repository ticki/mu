extern crate failure;
extern crate clap;
extern crate walkdir;

// TODO: Add help page for formatting of settings.mu files.

use failure::Error;
use clap::{Arg, App};
use walkdir::WalkDir;

use std::{fs, path, process};
use std::io::{self, Write, Read};
use std::ffi::OsStr;

/// Start mu with stdout `stdout`.
fn main_err() -> Result<(), Error> {
    // Lock stdout.
    let stdout = io::stdout();
    let mut stdout = stdout.lock();

    let matches = App::new("mkmu")
        .version("0.1.0")
        .about("Compiles a Mu deck from TeX files")
        .arg(Arg::with_name("OUTPUT")
             .help("the directory in which the deck will be stored")
             .default_value("deck"))
        .get_matches();

    // The output directory.
    let output_dir = path::PathBuf::from(matches.value_of("OUTPUT").unwrap());
    fs::create_dir_all(&output_dir)?;

    // The eventual content of the `.mu` deck file.
    let mut deck = String::new();

    // Load the settings, if it exists.
    match fs::File::open("settings.mu") {
        // The file does not exist. Do nothing.
        Err(ref err) if err.kind() == io::ErrorKind::NotFound => (),
        // The file exists. Read it.
        settings_file => { settings_file?.read_to_string(&mut deck)?; },
    }

    // Go over every entry in the current directory and compile the files.
    for entry in WalkDir::new(".") {
        // Throw error if necessary.
        let entry = entry?;
        if entry.file_type().is_file() {
            // The entry was a file.

            // Get the path.
            let path = entry.path();
            if path.extension() == Some(OsStr::new("tex")) {
                // Read TeX file.
                let mut tex_file = fs::File::open(&path)?;
                let mut tex = String::new();
                tex_file.read_to_string(&mut tex)?;

                // Skip if there is no metadata.
                if !tex.starts_with("%") {
                    writeln!(stdout, "Skipping {:?} due to lack of metadata (file must start with `%`)", path)?;
                    continue;
                }

                // Write section.
                deck.push_str(&format!("[card {}]\n", path.file_stem().unwrap().to_str().unwrap()));
                // Go over key-value pairs.
                for line in tex.lines().take_while(|line| line.starts_with("%")) {
                    // Rid the `%` starting the comment and trim spaces.
                    let line = line[1..].trim();
                    // Add the key-value pair.
                    deck.push_str(line);
                    // Append newline.
                    deck.push('\n');
                }

                // This is a TeX file.
                writeln!(stdout, "Compiling {:?}", path)?;

                // Run latexmk to compile the file.
                if !process::Command::new("latexmk")
                    // TODO: Don't do unwrap.
                    // Compile files into deck directory.
                    .arg(format!("-outdir={}", &output_dir.to_str().unwrap()))
                    // Set path to TeX file.
                    .arg(path.as_os_str())
                    // Start compilation.
                    .spawn()?
                    // Wait 'till it finishes.
                    .wait()?
                    // Determine if the command succeeded or not.
                    .success()
                {
                    // The command failed. Throw error.
                    return Err(failure::err_msg("Compilation failed."));
                }

                // TODO: Don't do unwrap.
                // Generate metadata from the first comments in the TeX file.
            }
        }
    }

    // Get path to the deck file.
    let mut deck_path = output_dir;
    deck_path.push("deck.mu");
    // Write the deck file.
    fs::write(deck_path, deck.as_bytes())?;

    Ok(())
}

fn main() -> Result<(), Error> {
    if let Err(err) = main_err() {
        // Handle errors.
        eprintln!("mkmu error: {}", err);
    }

    Ok(())
}

# `mu` — Advanced Unix-style Spaced Repetition System

After becoming increasingly dissatisfied with my workflow in Anki, which I used for studying, I
decided to write `mu`, which essentially replicates the functionality of Anki, but is more
"Unix-style":

- _A text-based, shell-like interface instead of a GUI:_ I hope I need not convince you that using
  your keyboard is faster than using your mouse, and as such (at least for "advanced users")
  programs centered around text (which is most programs) ought to be TUIs.
- _First-class TeX integration:_ While Anki does support TeX, it feels like a hack with all sorts of
  issues with diagrams and so on. `mu` is literally just opening a PDF viewer (e.g. in some sort of
  presentation mode, as `zathura` implements) for viewing card, and therefore will just view your
  rendered TeX file.
- _Flashcards sit in the file system instead of a database:_ One major issue with Anki is that
  flashcards are stored in a database file, which in particular means that you cannot keep your
  cards in a Git repository.
- _Editing is done in your editor of choice:_ Somewhat adjacent to the previous point, I found that
  the Anki editor encouraged laziness, because I could not use my editor of choice when creating and
  editing cards. When I have increased control, I care more and as a result I don't get a deck that
  is just thrown together.

## Technical details

`mu` is based on SM2 algorithms, specifically Anki's version of the algorithm (most of the defaults
are shamelessly stolen from Anki), with the exception of one important feature: adaptivity.

Adaptivity means that each tag is assigned a "familiarity" (which can be thought of as a tag-level
E-factor), which is adjusted based on your performance on the cards in the tags. When answering well
on a card in some tag, this factor increases, causing the intervals of cards with the tag to
increase more after review. It reflects the fact that you may be more familiar with one subject than
another.

## To-do

- [ ] Make interactive cards easier: While the `.mu` format implements `sh: ` followed by a command
  for specifying an arbitrary command (e.g. some program doing cloze) instead of a PDF, this is not
  implemented by `mkmu`.
  - [ ] Implement a `cloze` program and other interactive flashcard styles.
- [ ] Heatmap of activity.
- [ ] Align `rt` table.
- [ ] Add command to output sorted familiarities.
- [ ] Better, non-automated control over intervals:
  - [ ] Postponing cards

## Usage

You have a directory like the following:

    ├── gauss_bonnett.tex
    ├── schrodinger_equation.tex
    └── yoneda_lemma.tex

where each `.tex` file starts with something like:

    %tags: Fact, Noncourse
    %priority: 3

where `tags` are some user chosen, arbitrarily named strings separated by commas, and `priority` is
a number `1-5`, with a higher number meaning more important (both of these are used in determining
calculation of intervals).

You can then run `mkmu`, which crawls directories (and subdirectories) and compiles the TeX files
using the `latexmk` build tool. The resulting files are placed in the `deck` directory.

You can then run `mu` in the directory containing the `deck/` directory, which starts `mu`, entering
into a shell-like problem that looks like this (run `help` to see list of commands):

    ——— card 'grassmanian' ———
    file:      grassmanian.pdf
    tags:      Fact, W5
    priority:  2
    (opening grassmanian.pdf)
    ——— new intervals ———
    fail:  30m
    hard:  30m
    okay:  30m
    good:  1d
    easy:  1d
    D:0 N:9 >> good

This opens the TeX files, containing the flashcards. The default viewer is `zathura`, but can be set
by changing the `MU_PDF_VIEWER` environment variable.

# Design of Pachinko

Pachinko started a very long time (and several names) ago as a system for organizing my filing
cabinet.

## History

I was sick and tired of coming up with ways to organize my files. Everyone who's tried to keep
important papers around has run into something like the following:

* "Okay, I have my 'College' folder for school stuff. My student loan stuff goes in there."
* "My W-2s from last year can go in the '2019 Taxes' folder."
* "... now I have a document showing that I paid tuition last year, which I need to keep for this
  deduction... does it go in the 'Taxes' or 'College' folder?"

Librarians have struggled for centuries to come up with a Perfect Information Hierarchy, and they're
still arguing about it. I know I'm not going to be the genius that cracks the code, and worse, it's
utterly unnecessary.

Two things really matter for important stuff:
* I know roughly where it is.
* Whatever folder it's in shouldn't be so full that I can't dig it out.

The exact hierarchy it's in doesn't matter. If I just had a set of numbered folders, spread
everything evenly between them, and wrote down where everything ended up, all would be well.

A computer can do all of this in its sleep.

### Dead Ends

I tried several different UXs and UIs for this before getting to Pachinko:

#### Content-addressed CLI
The first version used a content-addressed hash identifier for each item. The hope was that the
   organization could be rebuilt if the database was lost. This system had some issues:

* It was too hard to come up with a hashing scheme that wasn't sensitive to word order or spelling.
* The first digit of the resulting hash was used to pick a folder. This meant that folder
    assignment was effectively random, and meant some folders were left uselessly empty.  
* The hex IDs were unwieldy. I tried several pronounceable-hash systems before giving up.

#### Web UI
The second version was a web UI with two big changes:
  * The content-aware hashes were completely abandoned. Picking items with a UI meant that their ID
    wasn't all that important.
  * Items' size was recorded, and new items were assigned a bin based on how full each bin was.

This was far more effective, and worked well for both paper in filing cabinet and random junk in a
set of boxes.

I only abandoned this version because maintaining a webapp is a grueling treadmill. I also didn't
want to implement authentication. I refuse to deal with oAuth without a paycheck involved.

## Needs

Pachinko, the third version of this concept, had a few goals:

- Be a CLI. Web UIs have a ton of advantages, but I don't have the spare time to keep my nose to the
  webpack/React/Flux/Immer/Express/GraphQL/etc. grindstone anymore.
- As a CLI, be built in Rust. Most of my previous CLIs had been written in Python. I love Python
  deeply, but its startup delay is super painful for a non-interactive CLI. Plus I really wanted to
learn Rust.
- Be an early adopter of my SQlite-based document store,
  [Qualia](https://github.com/pianohacker/qualia). Qualia began as a metadata-based tracker for
**computer** files, and its store ended up being the most interesting piece.

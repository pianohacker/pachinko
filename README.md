# Pachinko

Pachinko is a simple CLI for managing collections of physical things.

# Usage

Track what items you keep in a set of different locations:

```console
$ pachinko add-location Kitchen
$ pachinko add-location Bedroom
$ pachinko add Bedroom Necklace
Bedroom: Necklace (S)
$ pachinko add Kitchen Safe L
Kitchen: Safe (L)
$ pachinko items
Bedroom: Necklace (S)
Kitchen: Safe (L)
```

The `S` and `L` above refer to the size of each item. Items can be **S**mall, **M**edium, **Large**,
or E**X**tra-Large.

This matters if you use Pachinko in auto-sorting mode:
```
$ pachinko add-location Drawers 2
$ pachinko add Drawers "Pillow" X
Drawers/1: Pillow (X)
$ pachinko add Drawers "Trinket" S
Drawers/2: Trinket (S)
$ pachinko add Drawers "Laser Pointer" S
Drawers/2: Laser Pointer (S)
```

If you tell Pachinko that a location has a certain number of bins (2, in the above example) it will
automatically assign each new item to the least-full bin.

# Installation

```console
$ cargo install pachinko
```

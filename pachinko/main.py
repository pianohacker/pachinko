# Copyright (c) 2020 Jesse Weaver.
#
# This file is part of Pachinko.
# 
# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at https://mozilla.org/MPL/2.0/.

import click
from os import path
import qualia
import os 
import sys
import xdg

def _default_pachinko_dir():
	return str(xdg.XDG_DATA_HOME / 'pachinko')

@click.group()
@click.option(
	'--data-dir',
	type = click.Path(
		file_okay = False,
		writable = True,
	),
	default = _default_pachinko_dir(),
	envvar = ['PACHINKO_DIR'],
	help = 'Data directory [$PACHINKO_DIR].',
)
@click.pass_context
def _pachinko(ctx, data_dir):
	os.makedirs(data_dir, exist_ok = True)
	ctx.obj['store'] = qualia.open(path.join(data_dir, 'pachinko.qualia'))

@_pachinko.command()
@click.pass_context
def undo(ctx):
	ctx.obj['store'].undo()

@_pachinko.command("add-location")
@click.argument('NAME')
@click.argument('NUMBER_OF_BINS', type = click.IntRange(min = 1))
@click.pass_context
def add_location(ctx, name, number_of_bins):
	ctx.obj['store'].add(type = 'location', name = name, num_bins = number_of_bins)
	ctx.obj['store'].commit()

@_pachinko.command()
@click.pass_context
def locations(ctx):
	for location in ctx.obj['store'].select(type = 'location'):
		print(f'{location["name"]} ({location["num_bins"]} bins)')

def main():
	_pachinko(obj = {})

if __name__ == '__main__':
	main()

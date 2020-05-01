# Copyright (c) 2020 Jesse Weaver.
#
# This file is part of Pachinko.
# 
# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at https://mozilla.org/MPL/2.0/.

import click
import functools
import itertools
from os import path
import qualia
from qualia import query
import os 
import random
import re
import sys
import xdg

def _default_pachinko_dir():
	return str(xdg.XDG_DATA_HOME / 'pachinko')

SIZE_KEY = {'S': 1, 'M': 1.5, 'L': 2, 'X': 3}

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

def _pass_store(func):
	@click.pass_context
	@functools.wraps(func)
	def wrapper(ctx, *args, **kwargs):
		return func(ctx, ctx.obj['store'], *args, **kwargs)

	return wrapper

@_pachinko.command()
@_pass_store
def undo(ctx, store):
	store.undo()

@_pachinko.command("add-location")
@click.argument('NAME')
@click.argument('NUMBER_OF_BINS', type = click.IntRange(min = 1))
@_pass_store
def add_location(ctx, store, name, number_of_bins):
	store.add(type = 'location', name = name, num_bins = number_of_bins)
	store.commit()

@_pachinko.command()
@_pass_store
def locations(ctx, store):
	for location in store.select(type = 'location'):
		print(f'{location["name"]} ({location["num_bins"]} bins)')

def _find_bin(store, location, size):
	bin_weights = {bin_no: 0 for bin_no in range(1, location['num_bins'] + 1)}

	for item in store.select(type = 'item', location_id = location['object_id']):
		bin_weights[item['bin_no']] += SIZE_KEY[item['size']]
	
	_, min_weight = min(bin_weights.items(), key = lambda kv: kv[1])

	return random.choice([bin_no for (bin_no, weight) in bin_weights.items() if weight == min_weight])

@_pachinko.command("add")
@click.argument('location_bin', metavar = 'LOCATION[/BIN]')
@click.argument('NAME')
@click.argument('SIZE', type = click.Choice(['S', 'M', 'X', 'L']), default = 'S')
@_pass_store
def add(ctx, store, location_bin, name, size):
	match = re.match("([^/]+)(?:\/([1-9]\d*))?", location_bin)
	if match is None:
		raise click.BadParameter('Expected a location name (/ an optional bin number)', param_hint = 'LOCATION[/BIN]')

	location_name, bin = match.groups()

	location = list(store.query(query.PhraseQuery('name', location_name)))[0]

	if bin is None:
		bin = _find_bin(store, location, size)

	store.add(type = 'item', location_id = location['object_id'], bin_no = int(bin), name = name, size = size)

	store.commit()

@_pachinko.command("items")
@_pass_store
def items(ctx, store):
	items = []
	for item in store.select(type = 'item'):
		location = list(store.query(query.EqualityQuery('object_id', item['location_id'])))[0]
		items.append({
			"location_name": location["name"],
			"item_bin_no": item["bin_no"],
			"item_name": item["name"],
			"item_size": item["size"],
		})

	for item in sorted(items, key = lambda item: (item["location_name"], item["item_bin_no"], item["item_name"].lower())):
		print('{location_name}/{item_bin_no}: {item_name} ({item_size})'.format(**item))

def main():
	_pachinko(obj = {})

if __name__ == '__main__':
	main()

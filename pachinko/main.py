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

class LocationBinParamType(click.ParamType):
	def convert(self, value, param, ctx):
		if isinstance(value, tuple):
			return value

		match = re.match("([^/]+)(?:\/([1-9]\d*))?", value)
		if match is None:
			self.fail('Expected a location name (/ an optional bin number)')

		location_name, bin = match.groups()

		locations = list(ctx.obj['store'].query(
			query.AndQueries(
				query.EqualityQuery('type', 'location'),
				query.PhraseQuery('name', location_name),
			)
		))

		if len(locations) == 0:
			self.fail('Did not find a matching location')

		if len(locations) > 1:
			self.fail(f'Found multiple matching locations: {locations!r}')

		if bin and int(bin) > locations[0]["num_bins"]:
			self.fail(f'Bin out of range (1-{locations[0]["num_bins"]}) for given location')

		return locations[0], bin

def print_item(item):
	print('{location_name}/{item_bin_no}: {item_name} ({item_size})'.format(**item))

@_pachinko.command("add")
@click.argument('location_bin', metavar = 'LOCATION[/BIN]', type = LocationBinParamType())
@click.argument('NAME')
@click.argument('SIZE', type = click.Choice(['S', 'M', 'X', 'L']), default = 'S')
@_pass_store
def add(ctx, store, location_bin, name, size):
	location, bin = location_bin

	if bin is None:
		bin = _find_bin(store, location, size)

	item = dict(type = 'item', location_id = location['object_id'], bin_no = int(bin), name = name, size = size)

	store.add(**item)

	store.commit()

	print_item(dict((('item_' + k, v) for (k,v) in item.items()), location_name = location['name']))

@_pachinko.command("quickadd")
@click.argument('location_bin', metavar = 'LOCATION[/BIN]', type = LocationBinParamType())
@_pass_store
def quickadd(ctx, store, location_bin):
	location, bin = location_bin

	prompt = f'{location["name"]}> ' if bin is None else f'{location["name"]}/{bin}> '
	while True:
		try:
			item = input(prompt)
		except EOFError:
			break

		if item == '':
			break

		size = 'S'
		match = re.match("(.*?)\s+([SMLX])$", item)
		if match is not None:
			item, size = match.groups()

		ctx.forward(add, name = item, size = size)

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
		print_item(item)

def main():
	_pachinko(obj = {})

if __name__ == '__main__':
	main()

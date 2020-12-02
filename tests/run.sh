#!/bin/bash
#
## License
#
# Copyright (c) 2020 Jesse Weaver.
#
# This file is part of pachinko.
# 
# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at https://mozilla.org/MPL/2.0/.

## Setup
# Fail on errors or uninitialized variables,
set -eu
# and propagate up errors in pipes and command substitutions.
set -o pipefail

script_dir="$(cd $(dirname $BASH_SOURCE[0]); echo $PWD)"

# Create a temporary directory to run tests in that we'll delete later.
export TEST_FILE="$(realpath ${BASH_SOURCE[0]})"
export TEST_DIR="$(mktemp -d -t pachinko-tests.XXXXXXXX)"

export RUST_BACKTRACE=1
export RUST_LIB_BACKTRACE=1
if [[ -n ${RUST_NIGHTLY:-} ]]; then
	rustup run nightly cargo build ${CARGO_FLAGS:---release}
else
	cargo build ${CARGO_FLAGS:---release}
fi
PACHINKO="$(cargo metadata --format-version 1 | jq -r .target_directory)"/release/pachinko

cd $TEST_DIR
trap "rm -rf $TEST_DIR" EXIT

## Shared functions
if [[ -t 1 ]]; then
	function _wrap_if_tty() {
		echo "$1$3$2"
	}
else
	function _wrap_if_tty() {
		echo "$3"
	}
fi

function error_text() {
	_wrap_if_tty $'\e[31m' $'\e[0m' "$@"
}

function success_text() {
	_wrap_if_tty $'\e[32m' $'\e[0m' "$@"
}

function skip_text() {
	_wrap_if_tty $'\e[33m' $'\e[0m' "$@"
}

function _assert_failed() {
	echo "assertion failed: $1" >&2

	echo "backtrace:" >&2
	for i in $(seq 1 $(( ${#FUNCNAME[*]} - 1 ))); do
		echo "  ${FUNCNAME[$i]}:${BASH_LINENO[$(( i - 1 ))]}" >&2
	done

	exit 1
}

function assert_match() {
	if ! [[ "$1" =~ $2 ]]; then
		_assert_failed "'$1' !~ /$2/" 
	fi
}

function assert_equal() {
	if [[ "$1" != "$2" ]]; then
		_assert_failed "'$1' != '$2'" 
	fi
}

function assert_not_equal() {
	if [[ "$1" == "$2" ]]; then
		_assert_failed "'$1' == '$2'" 
	fi
}

function assert_pch() {
	eval "$PACHINKO $1" || _assert_failed "pachinko succeeds with flags '$1'"
}

function assert_pch_fails() {
	! "$PACHINKO $1" || _assert_failed "pachinko fails with flags '$1'"
}

function assert_pch_equal() {
	local result
	result="$(assert_pch "$1")" || exit 1
	assert_equal "$result" "$2"
}

function assert_pch_not_equal() {
	local result
	result="$(assert_pch "$1")" || exit 1
	assert_not_equal "$result" "$2"
}

function assert_pch_match() {
	local result
	result="$(assert_pch "$1")" || exit 1
	assert_match "$result" "$2"
}

function assert_pch_fails_matching() {
	error_text="$(! $PACHINKO $1 2>&1)" || _assert_failed "pachinko fails with flags '$1'"
	echo $error_text

	assert_match "$error_text" "$2"
}

## Tests
function test_there_should_be_no_locations_to_start() {
	assert_pch_equal "locations" ""
}

function test_an_added_location_should_be_visible() {
	assert_pch_match "add-location Test 16" ""
	assert_pch_match "locations" "Test \(16 bins\)"
}

function _setup_example_locations() {
	assert_pch "add-location Test 4"
	assert_pch "add-location Tiny 1"
	assert_pch "add-location Huge 16"
}

function test_adding_an_item_to_a_specified_bin() {
	_setup_example_locations

	assert_pch_match 'add Test/4 "Test item"' 'Test/4: Test item'
	assert_pch_match 'items' 'Test/4: Test item'
}

function test_adding_an_item_should_match_locations_case_insensitively() {
	_setup_example_locations

	assert_pch 'add test/4 "Test item"'
	assert_pch_match 'items' 'Test/4: Test item'
}
	
function test_adding_items_should_default_to_small_size() {
	_setup_example_locations

	assert_pch_match 'add test/4 "Test item"' 'Test/4: Test item \(S\)'
}
	
function test_adding_items_should_respect_the_given_size() {
	_setup_example_locations

	assert_pch_match 'add test/4 "Test item" M' 'Test/4: Test item \(M\)'
}

function test_items_should_sort_by_location,_then_bin,_then_alphabetically() {
	_setup_example_locations

	assert_pch 'add test/4 "Test item" M'
	assert_pch 'add test/3 "Test item" M'
	assert_pch 'add huge/6 "Test item" M'
	assert_pch "add test/4 \"Test blight'em\" M"

	assert_pch_match 'items' "Huge/6: Test item.*\
Test/3: Test item.*\
Test/4: Test blight'em.*\
Test/4: Test item"
}

function test_adding_an_item_without_a_bin_should_place_it_in_a_random_slot() {
	_setup_example_locations

	assert_pch_match "add test \"Test item\"" "Test/[1-4]: Test item .*"
}

function test_items_should_distribute_evenly() {
	_setup_example_locations

	assert_pch "add test \"Test item\""
	assert_pch "add test \"Test item\""
	assert_pch "add test \"Test item\""
	assert_pch "add test \"Test item\""

	assert_pch_match "items" "Test/1: Test item .*\
Test/2: Test item .*\
Test/3: Test item .*\
Test/4: Test item .*\
"
}

function test_items_should_distribute_to_the_most_empty_slot() {
	_setup_example_locations

	assert_pch "add test/1 \"M\" M"
	assert_pch "add test/2 \"S\" S"
	assert_pch "add test/3 \"L\" L"
	assert_pch "add test/4 \"X\" X"

	assert_pch_match "add test \"X2\" X" "Test/2: X2 .*"
	assert_pch_match "add test \"X3\" X" "Test/1: X3 .*"
}

function test_items_should_distribute_to_the_first_possible_slot() {
	_setup_example_locations

	assert_pch "add test/2 \"L\" L"

	assert_pch_match "add test \"X2\" X" "Test/1: X2 .*"
	assert_pch_match "add test \"X3\" X" "Test/3: X3 .*"
}

function test_quick_addition_into_random_bins() {
	_setup_example_locations

	echo 'Test 1
Test 2
Test 3 M' | assert_pch_match 'quickadd Test' 'Test/[1234]: Test 1 \(S\)
Test/[1234]: Test 2 \(S\)
Test/[1234]: Test 3 \(M\)'
}

## Test running loop
function get_test_functions() {
	awk '/^function test_/ { print $2 }' "${TEST_FILE}" | sed -e 's/()//'
}

# If any test functions are named FOCUS, default to focusing those.
if get_test_functions | grep FOCUS; then
	: ${FOCUS:=FOCUS}
fi

focus_filter=${FOCUS:-'^.*$'}

## Test running loop
for test_function in $(awk '/^function test_/ { print $2 }' "${TEST_FILE}" | sed -e 's/()//'); do
	test_name="$(sed -e 's/^test_//;s/_/ /g' <<<"$test_function")"

	if ! [[ "$test_name" =~ $focus_filter ]]; then
		echo "$(skip_text '[SKIP]') $test_name"
		continue
	fi

	if result=$(
		cd "$(TMPDIR=$TEST_DIR mktemp -d -t $test_function.XXXXXXXX)"

		export PACHINKO_STORE_PATH="$PWD/pachinko.qualia"

		exec 2>&1
		$test_function
	); then
		echo "$(success_text '[OK]') $test_name" 

		if [[ ${TEST_VERBOSITY:-} -ge 1 ]]; then
			echo "$result" | sed -e 's/^/    /'
		fi
	else
		echo "$(error_text '[FAILED]') $test_name"
		if [[ ${TEST_VERBOSITY:-} -ge 0 ]]; then
			echo "$result" | sed -e 's/^/    /'
		fi
		if [[ -z ${KEEP_GOING:-} ]]; then
			exit 1
		fi
	fi
done

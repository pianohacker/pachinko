from behave import given, when, then, use_step_matcher
from hamcrest import *
import io
import os
from os import path
import shlex
import shutil
import sys
import tempfile

from pachinko import main

def _get_temp_directory(context):
	dir = tempfile.mkdtemp(prefix = 'pachinko-tests-')

	context.add_cleanup(lambda: shutil.rmtree(dir))

	return dir

@given('an empty data directory')
def step_impl(context):
	pachinko_dir = _get_temp_directory(context)
	os.environ['PACHINKO_DIR'] = pachinko_dir

use_step_matcher('re')

def _run_command(context, arguments, input = None):
	old_stdin = sys.stdin
	if input is not None:
		sys.stdin = io.StringIO(input)

	old_stdout = sys.stdout
	sys.stdout = io.StringIO()
	old_stderr = sys.stderr
	sys.stderr = io.StringIO()
	old_argv = sys.argv
	sys.argv = ['pachinko'] + shlex.split(arguments)

	try:
		main.main()

		context.exit_code = 0
	except SystemExit as e:
		if isinstance(e.code, int):
			context.exit_code = e.code
		elif e.code is not None:
			context.exit_code = 1
	finally:
		context.stdout = sys.stdout.getvalue()
		context.stderr = sys.stderr.getvalue()

		sys.stdin = old_stdin
		sys.stdout = old_stdout
		sys.stderr = old_stderr
		sys.argv = old_argv

@when(r'we execute `(?P<arguments>[^`]*)`')
def step_impl(context, arguments):
	_run_command(context, arguments)

@when(r'we execute `(?P<arguments>[^`]*)` with the input')
def step_impl(context, arguments):
	_run_command(context, arguments, input = context.text)

@then(r'we expect `(?P<match>[^`]*)`')
def step_impl(context, match):
	assert_that(context.stdout, matches_regexp(match))
	assert_that(context.stderr, empty())

@then(r'we expect the output `(?P<match>[^`]*)`')
def step_impl(context, match):
	assert_that(context.stdout, matches_regexp(match))

@then(r'we expect the error `(?P<match>[^`]*)`')
def step_impl(context, match):
	assert_that(context.stderr, matches_regexp(match))

@then('we expect no output')
def step_impl(context):
	assert_that(context.stdout, empty())

@then('we expect no errors')
def step_impl(context):
	assert_that(context.stderr, empty())

@then('we expect a nonzero exit code')
def step_impl(context):
	assert_that(context.exit_code, is_not(0))

@then('we expect success')
def step_impl(context):
	assert_that(context.exit_code, is_(0))

@then('we expect silent success')
def step_impl(context):
	assert_that(context.stderr, empty())
	assert_that(context.stdout, empty())
	assert_that(context.exit_code, is_(0))

from behave import given, when, then, use_step_matcher
from hamcrest import *
import io
from os import path
import shlex
import shutil
import sys
import tempfile

from pachinko import main

def _get_temp_directory(context):
	dir = tempfile.mkdtemp(prefix = 'qualia-tests-')

	context.add_cleanup(lambda: shutil.rmtree(dir))

	return dir

@given('an empty data directory')
def step_impl(context):
	pachinko_dir = _get_temp_directory(context)
	os.environ['PACHINKO_DIR'] = pachinko_dir

use_step_matcher('re')

@when(r'we execute `(?P<arguments>[^`]*)`')
def step_impl(context, arguments):
	old_stdout = sys.stdout
	sys.stdout = io.StringIO()
	old_stderr = sys.stderr
	sys.stderr = io.StringIO()
	old_argv = sys.argv
	sys.argv = ['pachinko'] + shlex.split(arguments)

	try:
		main.main()

		context.stdout = sys.stdout.getvalue()
		context.stderr = sys.stderr.getvalue()
	finally:
		sys.stdout = old_stdout
		sys.stderr = old_stderr
		sys.argv = old_argv

@then(r'the output contains `(?P<match>[^`]*)`')
def step_impl(context, match):
	assert_that(context.stdout, matches_regexp(match))

Feature: Usage messages
	Scenario: Running the command with no arguments should print usage
		When we execute ``
		Then we expect `Usage: .*pachinko`
		And we expect `--data-dir`
		And we expect no errors
	
	Scenario: Running a nonexistent subcommand should print usage on stderr
		When we execute `potato`
		Then we expect a nonzero exit code
		And we expect the error `Usage: .*pachinko`
		And we expect the error `potato`
		And we expect no output
	

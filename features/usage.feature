Feature: Usage messages
	Scenario: Running the command with no arguments should print usage
		Given an empty data directory
		When we execute ``
		Then the output contains `Usage: .*pachinko`

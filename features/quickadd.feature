Feature: Quick-add
	Background:
		Given an empty data directory
		When we execute `add-location Test 4`

	Scenario: quick addition into random bins
		When we execute `quickadd Test` with the input:
			"""
			Test 1
			Test 2
			Test 3 M
			"""

		Then we expect `Test> Test/\d: Test 1 \(S\)\nTest> Test/\d: Test 2 \(S\)\nTest> Test/\d: Test 3 \(M\)`

	Scenario: quick addition into random bins
		When we execute `quickadd Test/2` with the input:
			"""
			Test 1
			Test 2
			Test 3 M
			"""

		Then we expect `Test/2> Test/2: Test 1 \(S\)\nTest/2> Test/2: Test 2 \(S\)\nTest/2> Test/2: Test 3 \(M\)`


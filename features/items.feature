Feature: items
	Background:
		Given an empty data directory
		When we execute `add-location Test 4`
		When we execute `add-location Tiny 1`
		When we execute `add-location Huge 16`
		Then we expect silent success

	Scenario: adding an item to a specified bin
		When we execute `add Test/4 "Test item"`
		Then we expect `Test/4: Test item`

		When we execute `items`
		Then we expect `Test/4: Test item`

	Scenario: adding an item should match locations case insensitively
		When we execute `add test/4 "Test item"`
		Then we expect success

		When we execute `items`
		Then we expect `Test/4: Test item`
	
	Scenario: adding items should default to small size
		When we execute `add test/4 "Test item"`
		Then we expect `Test/4: Test item \(S\)`
	
	Scenario: adding items should respect the given size
		When we execute `add test/4 "Test item" M`
		Then we expect `Test/4: Test item \(M\)`

	Scenario: items should sort by location, then bin, then alphabetically
		When we execute `add test/4 "Test item" M`
		Then we expect success
		When we execute `add test/3 "Test item" M`
		Then we expect success
		When we execute `add huge/6 "Test item" M`
		Then we expect success
		When we execute `add test/4 "Test blight'em" M`
		Then we expect success

		When we execute `items`
		Then we expect `Huge/6: Test item.*\nTest/3: Test item.*\nTest/4: Test blight'em.*\nTest/4: Test item`

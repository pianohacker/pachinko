Feature: autosorting
	Background:
		Given an empty data directory
		When we execute `add-location Test 4`
		When we execute `add-location Tiny 1`
		When we execute `add-location Huge 16`
		Then we expect silent success
	
	Scenario: adding an item without a bin should place it in a random slot
		When we execute `add test "Test item"`
		Then we expect `Test/[1-4]: Test item .*`
	
	Scenario: items should distribute evenly
		When we execute `add test "Test item"`
		Then we expect success
		When we execute `add test "Test item"`
		Then we expect success
		When we execute `add test "Test item"`
		Then we expect success
		When we execute `add test "Test item"`
		Then we expect success

		When we execute `items`
		Then we expect `Test/1: Test item .*\nTest/2: Test item .*\nTest/3: Test item .*\nTest/4: Test item .*\n`

	Scenario: items should distribute to the most empty slot
		When we execute `add test/1 "M" M`
		Then we expect success
		When we execute `add test/2 "S" S`
		Then we expect success
		When we execute `add test/3 "L" L`
		Then we expect success
		When we execute `add test/4 "X" X`
		Then we expect success

		When we execute `add test "X2" X`
		Then we expect `Test/2: X2 .*`

		When we execute `add test "X3" X`
		Then we expect `Test/1: X3 .*`

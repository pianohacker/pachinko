Feature: Locations
	Background:
		Given an empty data directory

	Scenario: There should be no locations to start
		When we execute `locations`
		Then we expect silent success
	
	Scenario: An added location should be visible
		When we execute `add-location Test 16`
		Then we expect silent success

		When we execute `locations`
		Then we expect `Test \(16 bins\)`
	
	Scenario: It should be possible to undo adding a location
		When we execute `add-location Test 16`
		Then we expect silent success

		When we execute `undo`
		Then we expect silent success

		When we execute `locations`
		Then we expect silent success

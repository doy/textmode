# Changelog

## [0.4.1] - 2025-01-30

### Changed

* updated dependencies

## [0.4.0] - 2023-03-08

### Fixed

* fixed textmode::blocking::Output::hard_refresh accidentally being async

### Changed

* migrated to tokio

## [0.3.0] - 2021-12-15

### Added

* `hide_cursor` to hide or show the cursor

### Changed

* combined `Error::SetRaw` and `Error::UnsetRaw` into a single
  `Error::SetTerminalMode` variant

## [0.2.2] - 2021-12-06

### Changed

* bump deps

## [0.2.1] - 2021-12-05

### Changed

* bump deps

## [0.2.0] - 2021-11-17

### Added

* `hard_refresh` to fully redraw the screen
* `move_relative` to move the cursor relative to its location

### Changed

* re-export `vt100::Color` to avoid requiring users to know about the internal
  `vt100` details (and potential version conflicts)

## [0.1.1] - 2021-11-10

### Changed

* Bumped deps and moved to 2021 edition

## [0.1.0] - 2021-03-13

### Added

* Initial release

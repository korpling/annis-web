# Changelog

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- Update to Bulma 1.0.0

## [0.2.0] - 2023-10-30

### Fixed

- Automatically create the SQLite file given with `--session-file` if it does
  not exist yet, instead of aborting.

### Changed

- Use Moka Cache for storing the sessions when no SQLite file is given.

## [0.1.0] - 2023-10-24

Initial release of annis-web.

### Added

- Basic CSV exporter with support for segmentations and context for the spanned
  text.
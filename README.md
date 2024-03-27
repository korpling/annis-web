[![codecov](https://codecov.io/gh/korpling/annis-web/graph/badge.svg?token=FX7LX6OA37)](https://codecov.io/gh/korpling/annis-web)


# ANNIS frontend experiments

ANNIS is an open source, versatile web browser-based search and visualization
architecture for complex multilevel linguistic corpora with diverse types of
annotation. This is an **experimental** version of ANNIS trying to
rethink the user interface and implementation of the ANNIS frontend.

## What is the difference to the original ANNIS version 4?

The experimental is very limited in its feature set, and only supports exporting
the results as CSV for now. Additional features, like other exporters or a
frequency analysis will be added later.

## Why starting from scratch?

ANNIS 4 is based on a web-frontend library called [Vaadin
8](https://vaadin.com/vaadin-8). Since Vaadin 8 is end-of-life and receives no
further updates, we have to rethink the whole technical application stack of the
ANNIS frontend. Updating to Vaadin 23 is practically a complete rewrite, so we
should be open about which Programming Languages and Frameworks we use.
Especially, porting the different visualizers might become a larger struggle.
This project is meant to create an experimental prototype with a Rust-based
framework and technologies and to create a usable next generation of the ANNIS
frontend step-by-step. While Vaadin 7/8 was around as Open Source for a long
time, we are even more conservative when choosing the new technology, so it will
work for a long time.

### Frameworks used

- htmx for dynamic updates of the UI <https://htmx.org/>
- axum web framework <https://github.com/tokio-rs/axum>
- minijinja template engine <https://github.com/mitsuhiko/minijinja>
- Bulma <https://bulma.io/> for styling

## Developing annis-web

You need to install Rust to compile the project.
We recommend installing the following Cargo subcommands for developing annis-web:

- [cargo-release](https://crates.io/crates/cargo-release) for creating releases
- [cargo-about](https://crates.io/crates/cargo-about) for re-generating the
  third party license file
- [cargo-watch](https://crates.io/crates/cargo-watch) allows automatic re-compilation
- [cargo-llvm-cov](https://crates.io/crates/cargo-llvm-cov) for determining the code coverage
- [cargo-insta](https://crates.io/crates/cargo-insta) allows reviewing the test snapshot files.

### Running the web server

When developing, you can run a web server that is automatically re-compiled when
any of the source files changes.

```bash
cargo watch -x 'run -- --session-file annis-frontend-sessions.db'
```

### Execute tests

You will need a Chromium/Chrome browser and the matching `chromedriver` binary
installed to execute the tests. Before running the tests, start `chromedriver`
on port 4444.

```bash
chromedriver --port=4444
```

If the Chromium/Chrome binary is installed as a snap, you might have to change
the temporary directory by setting the `TMPDIR` environment variable.

```bash
mkdir -p "${HOME}/tmp/"
TMPDIR="${HOME}/tmp/" chromedriver --port=4444
```

Then run the tests in another terminal.

```bash
cargo test
```

To execute the tests and calculate the code coverage, you can use `cargo-llvm-cov`:

```bash
cargo llvm-cov --open --ignore-filename-regex 'tests?\.rs'
```


### Compiling the CSS

Make sure you use Dart Sass to compile the Bulma-based ANNIS stylesheets.

```bash
sass --style compressed --source-map bulma-annis/sass/annis.scss static/annis.min.css
```
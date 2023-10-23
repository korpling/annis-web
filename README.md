# ANNIS frontend experiments

ANNIS is an open source, versatile web browser-based search and visualization
architecture for complex multilevel linguistic corpora with diverse types of
annotation. This is an **experimental** version of ANNIS trying to
rethink the user interface and implementation of the ANNIS frontend.

## What is the difference to the original ANNIS version 4?

The experimental is very limited in its feature set, and only supports exporting
the results as CSV for now. Additional features, like other exporters or a
frequency analysis will be added later.

## Why do start from scratch?

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
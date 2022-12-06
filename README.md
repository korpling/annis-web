# ANNIS frontend experiments

Since Vaadin 8 is end-of-life, we have to rethink the whole application stack of the ANNIS frontend.
Updating to Vaadin 23 is practically a complete rewrite, so we should be open about which Programming Languages and Frameworks we use.
Especially, porting the different visualizers might become a larger struggle.

This repository is meant to create prototypes with a Rust-based framework and technologies and to evaluate the feasibiltiy.
While Vaadin 7/8 was around as Open Source for a long time, we should be even more conservative when choosing the new technology, so it will work for a long time.


## Frameworks of interest

- Bulma <https://bulma.io/>
- htmx for dynamic updates of the UI <https://htmx.org/>
- axum web framework <https://github.com/tokio-rs/axum>
- askama template engine <https://github.com/djc/askama>

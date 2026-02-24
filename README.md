# Pharos GitHub Pages Site

This branch contains the static site content for Project Pharos, served at [iamrichardd.com/pharos](https://iamrichardd.com/pharos).

## Structure
- `_config.yml`: Jekyll configuration.
- `index.md`: Home page.
- `docs/`: Documentation and guides.

## Local Development (Podman)
To preview the site locally:
```bash
podman run --rm -it -v "$PWD:/src" -p 4000:4000 jekyll/jekyll jekyll serve
```

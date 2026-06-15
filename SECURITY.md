# Security Policy

## Reporting a vulnerability

Please report security issues **privately**, not through public issues.

- Preferred: GitHub's [private vulnerability reporting](https://github.com/sjourdois/morpion-solitaire/security/advisories/new)
  (Security → Report a vulnerability).
- Or email <stephane@jourdois.fr>.

Please include a description, affected component (the `morpion-solitaire`
application, or the `morpion-solitaire-record` / `msr` library), and steps to
reproduce. We aim to acknowledge reports within a week and will keep you updated
on the fix and disclosure timeline.

## Scope

This is an offline puzzle solver, so the attack surface is small. The most
relevant areas are:

- **Parsing untrusted input** in the `msr` library (decoding `.msr` records:
  Base64, DEFLATE, JSON) and the Pentasol importer — a malformed file should
  return an error, never panic, hang, or execute code.
- The CLI reading files supplied on the command line.

The WebAssembly build runs entirely client-side in the browser sandbox.

## Supported versions

The project is pre-1.0; fixes land on `main` and in the next release. Once `0.1`
is published, only the latest minor version is supported.

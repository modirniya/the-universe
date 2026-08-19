# Security policy

## Scope, honestly

This is a research toy: a CLI that reads a local TOML file, runs a cellular
automaton, and writes a CSV and a JSON file. It opens no sockets, executes no
user-supplied code, deserializes nothing from the network, and has two direct
dependencies (`serde` and `toml`), both only for reading that config file.

The realistic attack surface is close to nil. This policy exists so that if you
do find something, you know where to send it — not because the project is
pretending to be infrastructure.

Things that would genuinely be worth reporting:

- A panic or memory-safety problem reachable from a config file, which matters
  to anyone running this on configs they did not write.
- A dependency advisory affecting the versions pinned in `Cargo.lock`.
- Anything in CI that could let a pull request exfiltrate secrets or write to
  the repository.

## Supported versions

`main` only. This project is pre-1.0 and there are no maintenance branches; a
fix lands on `main` and that is the release.

## Reporting

Please use GitHub's [private vulnerability reporting][pvr] on this repository,
which keeps the discussion private until a fix exists. If you would rather use
email: **modirniya@gmail.com**.

Please do not open a public issue for something exploitable.

Expect an acknowledgement within a week. This is a single-maintainer project,
so that is a realistic commitment rather than an optimistic one.

## What you can expect

I will confirm receipt, tell you whether I think it is a real issue and why,
and credit you when it is fixed unless you would rather I did not. If I decide
something is not a vulnerability, I will say so plainly and explain the
reasoning rather than letting the report go quiet.

[pvr]: https://github.com/modirniya/the-universe/security/advisories/new

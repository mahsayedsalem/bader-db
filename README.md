[![Build Status](https://img.shields.io/github/actions/workflow/status/mahsayedsalem/bader-db/quickstart.yml?branch=main)](https://github.com/mahsayedsalem/bader-db/actions)
[![Crates.io](https://img.shields.io/crates/v/bader-db.svg)](https://crates.io/crates/bader-io)

<h1 align="center">
  BADER-DB (بادِر)
</h1>

<h4 align="center">Key-value cache RESP server with support for key expirations 🏪</h4>

<p align="center">
  <a href="#supported-features">Supported Features</a>
</p>

## Supported Features

* SET — Set with or without an expiry date.
* GET — Get value by key, we store our values in a BTree.
* Key Eviction 🏪 — A memory-efficient probabilistic eviction algorithm similar to [Redis](https://redis.io/commands/expire).
  

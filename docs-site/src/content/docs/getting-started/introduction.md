---
title: What is VASSL?
description: An overview of VASSL — Video Access Security Solutions Ltd's internal operations platform.
---

VASSL is Video Access Security Solutions Ltd's internal operations platform. It runs as a native desktop application on macOS and Windows, giving the operations team a single place to manage:

- **Inventory** — products, stock levels, and restocking
- **Price Book** — cost prices, duty, markup, and selling prices
- **Quotations** — project-based client quotations with line items
- **Suppliers** — supplier contacts and preferred supplier assignments

All data is stored locally in a SQLite database. There is no cloud sync — data lives on the machine where VASSL is installed.

## Who is it for?

VASSL is built exclusively for Video Access Security Solutions Ltd internal staff. It is not a public product.

## Technology

VASSL is built with:

- **Rust** — systems language for performance and reliability
- **GPUI** — GPU-accelerated UI framework (the same framework that powers [Zed](https://zed.dev))
- **SQLite** — embedded database via the `sqlez` library

## Release channels

| Version format | Channel | Database |
|---|---|---|
| `0.x.y` | Dev | `0-dev/vassl.db` |
| `x.y.z-preview[.N]` | Preview | `0-preview/vassl.db` |
| `x.y.z` (major ≥ 1) | Stable | `0-stable/vassl.db` |

Each channel uses a separate database so dev and stable builds can coexist on the same machine without affecting each other.

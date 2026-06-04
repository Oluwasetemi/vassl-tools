---
title: Audit Log
description: Tracking all changes made in VASSL.
---

The Audit Log records every create, update, and delete action taken in VASSL, along with who made the change and when.

Open with **Cmd+Shift+A** (macOS) / **Ctrl+Shift+A** (Windows), or via **File → Audit Log** in the menu bar.

## Log columns

| Column | Description |
|---|---|
| Table | Which module the record belongs to (e.g. `products`, `quotations`) |
| Record ID | The database ID of the affected record |
| Action | `INSERT`, `UPDATE`, or `DELETE` |
| Changed By | The user name set during First Run |
| Changed At | Timestamp of the change |

## Use cases

- **Accountability** — see who added or changed a price entry
- **Debugging** — trace unexpected stock or price changes
- **Compliance** — keep a history of quotation approvals

## Notes

- The audit log is read-only — records cannot be edited or deleted
- Logs are stored in the same SQLite database as all other data
- There is currently no export function; use a SQLite browser (e.g. [DB Browser for SQLite](https://sqlitebrowser.org)) to query the `audit_log` table directly if needed

## Diagrams

<!-- TODO: Add screenshot of the Audit Log panel -->

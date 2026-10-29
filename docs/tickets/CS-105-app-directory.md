# CS-105 — An app directory

**Wave:** 28 — what 4.0.0 needs (2026-10-29 feedback on 3.7.0)
**Area:** frontend · backend · docs
**Blocked by:** CS-103, CS-104
**Blocks:** —

## Problem

What a workspace can connect is spread over four places: connected apps (Jira, Confluence,
GitHub, Google Drive, Figma) in one section, webhooks and slash commands in *Integrations*,
workflows in their own view, and after CS-103 the workspace's own apps. Nobody can see in one
place what is available, what is installed, what each thing may do, and who added it.

## Approach

- **Workspace settings → Apps** becomes the directory. *Available*: the built-in connected
  apps and *Build an app*. *Installed*: every connected app, workspace app, webhook, slash
  command and workflow, with who added it, when, its scopes or permissions, the channels it
  touches, and whether it is working (CS-074's `disabled_reason`, CS-104's failing state).
- **One page per app** with what it can do, *Reinstall*, *Rotate token* and *Remove*.
- **Installing asks for consent**: the scopes in plain words before the token exists, and a
  change of scopes asks again.
- Members see the directory read-only and can ask an admin to add something, which lands in
  the admins' notifications.
- The channel's *Integrations* section links each entry to its directory page.

## Acceptance

- [ ] Everything connected to a workspace is listed in one place with its owner, its
  permissions and its health.
- [ ] Nothing gets a token or a connection without an admin consenting to its scopes.
- [ ] A member can see what is installed and request an app.

## Tests

API: the listing covers every kind with its owner and health, requests notify admins.
Frontend: the directory, an app page, the consent step, the member's read-only view.

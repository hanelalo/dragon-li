# Proposal: Skill Management UI

**Goal**: Provide a desktop UI for users to view, toggle, and manage local skills.

**Context**: Users need visibility into which skills the system has discovered, their descriptions, and a way to enable or disable them. They also need a quick way to open the local folder to edit the `SKILL.md`.

**Inputs**: SQLite `capabilities` table records where `type='skill'`.
**Outputs**: A new `SkillPage.vue` in the desktop app.
# Proposal: Skill Discovery and DB Sync

**Goal**: Implement local skill discovery and SQLite synchronization based on AgentSkills specification.

**Context**: Skills are stored as `~/.dragon-li/skills/<name>/SKILL.md`. We need a mechanism to parse the YAML frontmatter (name, description, allowed-tools) and persist metadata into the `capabilities` table so the system is aware of them.

**Inputs**: Local directories in `~/.dragon-li/skills/`.
**Outputs**: Updated `capabilities` table in SQLite.
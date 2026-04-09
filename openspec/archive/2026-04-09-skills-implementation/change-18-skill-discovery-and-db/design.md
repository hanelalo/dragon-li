# Design: Skill Scanner and Storage

1. **Directory Structure**: Rely on `~/.dragon-li/skills/<name>/SKILL.md`.
2. **SkillManager (Python)**: Create a singleton `SkillManager` in `agent/skill_manager.py`. It will be responsible for scanning the directory, parsing the Markdown files, and caching the active skills.
3. **Parsing Logic**: Extract YAML Frontmatter (between `---` tags) to get `name`, `description`, and `allowed-tools`. Extract the rest as the Markdown body (SOP Instructions).
4. **DB Sync**: Upsert the parsed metadata into the SQLite `capabilities` table (`type='skill'`). Save `allowed-tools` and other metadata inside the `input_schema_json` field.
5. **Guardrails**: Remove the `skill_execute` block from `config_guardrails.rs` to allow the system to proceed with skill execution.
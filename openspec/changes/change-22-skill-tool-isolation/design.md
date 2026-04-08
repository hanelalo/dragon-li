# Design: Skill Execution Sandbox

1. **MCP Filtering**: When a skill is active, `llm_provider.py` asks `SkillManager` for tools. `SkillManager` reads the `allowed-tools` list from the skill's metadata and fetches only those specific tools from `McpClientManager`.
2. **Local Scripts Discovery**: `SkillManager` scans the skill's `scripts/` directory. For MVP, we can define a standard where scripts must output their JSON schema when run with a `--schema` flag.
3. **Execution Sandbox**: When the LLM calls a local script tool, `SkillManager.execute_local_tool()` uses `subprocess.run` to execute the script, strictly setting the `cwd` (Current Working Directory) to the skill's folder.
# Proposal: Skill Tool Isolation and Sandbox

**Goal**: Isolate tool access when a skill is active, decoupling from global MCP and allowing local script execution.

**Context**: A skill should only access the tools it explicitly requested (`allowed-tools` in YAML) or its own local scripts in the `scripts/` directory. It should not have access to the entire global MCP toolset.

**Inputs**: Active skill context in `llm_provider.py`.
**Outputs**: A filtered list of tools provided to the LLM.
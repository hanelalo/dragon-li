# Design: LLM Routing via Tool Call

1. **Dynamic Tool Schema**: `SkillManager` generates a `delegate_to_skill` tool schema. The `skill_name` parameter must be an `Enum` of currently active skills. The tool description must list all active skills and their descriptions.
2. **Tool Injection**: In `llm_provider.py`, inject this tool only when operating in general chat mode (i.e., no skill is currently active).
3. **Silent Interception**: When the LLM returns a `tool_call` for `delegate_to_skill`, intercept it. **Do not** yield this tool call to the frontend.
4. **Context Switch & Re-prompt**: Extract `skill_name` and `task_context` from the tool arguments. Load the target `SKILL.md` body as the new System Prompt. Fire a new LLM completion request using `task_context` as the user input, and stream that response to the user.
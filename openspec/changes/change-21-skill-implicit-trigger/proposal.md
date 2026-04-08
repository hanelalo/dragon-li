# Proposal: Skill Implicit Trigger (LLM Routing)

**Goal**: Enable the LLM to autonomously delegate tasks to specific skills via Tool Calling.

**Context**: When a user asks a complex question without an explicit `@mention`, the LLM should evaluate available skills and use a tool to switch context to the appropriate skill's SOP, passing along a refined task context.

**Inputs**: User prompt in general chat mode.
**Outputs**: Context switch to expert mode, followed by a new LLM stream response.
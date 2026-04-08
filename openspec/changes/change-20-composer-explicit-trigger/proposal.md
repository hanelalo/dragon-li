# Proposal: Composer Explicit Trigger (@mention)

**Goal**: Allow users to explicitly invoke a skill using an `@` mention in the chat input, sending the explicit intent to the backend.

**Context**: A standard `<textarea>` cannot treat `@web-dev` as a single atomic node. We need a rich text input to handle mentions elegantly. The backend needs to know when a skill is explicitly requested to bypass intent routing.

**Inputs**: User types `@` in the chat composer.
**Outputs**: Payload containing `{"text": "...", "explicit_skill_id": "web-dev"}` sent to the backend.
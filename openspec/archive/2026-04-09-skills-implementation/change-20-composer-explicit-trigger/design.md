# Design: Tiptap Composer & Explicit Trigger

1. **Rich Text Editor**: Replace `<textarea>` in `Composer.vue` with `tiptap`.
2. **Mention Extension**: Use `@tiptap/extension-mention`. Configure it to fetch the list of `enabled=1` skills.
3. **Atomic Nodes**: Ensure the mention is rendered as a chip/tag and is deleted as a whole when backspacing.
4. **Payload Modification**: On submit, parse the Tiptap document. Extract the plain text. If a skill mention exists, extract its ID and attach it to the `ChatRequestInput` payload as `explicit_skill_id`.
5. **Backend Handling**: In `llm_provider.py`, if `explicit_skill_id` is present, immediately load the target skill's Markdown body as the System Prompt.
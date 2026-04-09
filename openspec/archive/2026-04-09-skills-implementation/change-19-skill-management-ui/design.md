# Design: SkillPage.vue

1. **UI Layout**: Mirror the layout of `McpPage.vue`. Sidebar for the list of skills, main area for details.
2. **Tauri Commands**:
   - `skill_list`: Fetch all skills from the database.
   - `skill_toggle`: Update the `enabled` status of a specific skill.
   - `skill_rescan`: Trigger the Python agent to re-run the directory scan.
3. **Display Elements**: Show `name`, `description`, and an "Enabled" toggle switch.
4. **Action Buttons**: Add an "Open Folder" button that uses Tauri's `shell.open` API to open the local `~/.dragon-li/skills/<name>` directory in the OS file explorer.
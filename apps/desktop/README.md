# Dragon Li Desktop (Runtime Core)

## What is included

- Tauri + Vue desktop skeleton.
- Rust host commands:
  - Runtime: `ping`, `runtime_info`, `start_agent`, `stop_agent`, `agent_status`, `agent_health_check`
  - Config: `config_get`, `config_save`, `config_check_external_change`, `config_apply_external_change`
  - Guardrails: `guardrail_validate_path`, `guardrail_validate_capability`, `guardrail_validate_network`
  - SQLite: `db_init`, `session_create`, `session_list`, `message_create`, `message_list`, `request_log_create`, `request_log_list_by_request_id`, `session_soft_delete`, `session_restore`
  - Chat: `chat_send`
  - Memory Pipeline: `memory_extract_candidates`, `memory_list_candidates`, `memory_review_candidate` (`approve/reject/merge`), `memory_soft_delete`, `memory_restore`, `memory_read`, `memory_search`, `memory_list_long_term`

## macOS packaging

- Build package:
  - `bash /Users/hanelalo/develop/dragon-li/scripts/build-macos-package.sh aarch64`
- Verify package:
  - `bash /Users/hanelalo/develop/dragon-li/scripts/verify-macos-package.sh`
- Manual upgrade guide:
  - `/Users/hanelalo/develop/dragon-li/docs/release/macos-manual-upgrade.md`

## Quick verify

1. Install frontend deps:
   - `cd apps/desktop && npm install`
2. Start desktop app:
   - `cd apps/desktop && npm run dev`
3. Use UI to validate memory center workflow and runtime commands.

# 07. API Profiles JSON 示例

配置文件路径：

- `~/.dragon-li/config/api_profiles.json`

示例内容：

```json
{
  "version": 1,
  "updated_at": "2026-04-05T13:00:00+08:00",
  "profiles": [
    {
      "id": "openai-main",
      "name": "OpenAI Main",
      "provider": "openai",
      "base_url": "https://api.openai.com/v1",
      "api_key": "sk-xxxx",
      "default_model": "gpt-4.1",
      "enabled": true,
      "created_at": "2026-04-05T13:00:00+08:00",
      "updated_at": "2026-04-05T13:00:00+08:00"
    },
    {
      "id": "anthropic-main",
      "name": "Anthropic Main",
      "provider": "anthropic",
      "base_url": "https://api.anthropic.com",
      "api_key": "sk-ant-xxxx",
      "default_model": "claude-sonnet-4-5",
      "enabled": true,
      "created_at": "2026-04-05T13:00:00+08:00",
      "updated_at": "2026-04-05T13:00:00+08:00"
    }
  ]
}
```

说明：

- `provider` 在 MVP 中仅允许：`openai`、`anthropic`。
- `enabled=false` 的 profile 不参与模型选择与调用。
- 应用设置页与手动编辑该文件都可修改配置，保存时应进行 schema 校验并原子写入。

# 04. 配置与 API Profiles

## 配置目录

- `~/.dragon-li/config/`

## 配置文件

- `~/.dragon-li/config/api_profiles.json`

## 配置策略（已确定）

- 使用单一 JSON 文件管理多个 API Profile。
- `api_key` 明文存储（当前阶段不加密）。
- 用户可通过两种方式修改：
  - 在应用设置页面修改。
  - 直接编辑 `api_profiles.json`。

## Provider 范围（MVP）

- OpenAI
- Anthropic

## 读写约定

- 应用启动时加载配置文件。
- UI 修改后回写同一文件。
- 检测到文件外部改动时，显示提示并由用户确认后刷新配置。
- 保存采用原子写入，避免文件损坏。

## 热更新策略（已确定）

- 监听文件：`~/.dragon-li/config/api_profiles.json`
- 检测外部改动后，顶部提示：
  - `检测到配置文件变更，是否刷新并应用？`
- 用户点击“应用”后执行重载与生效。
- 应用前执行 JSON 与 schema 校验。
- 校验失败时：
  - 保持当前内存配置不变
  - 展示具体错误信息（支持查看详情）

## 建议字段

- `id`
- `name`
- `provider`
- `base_url`
- `api_key`
- `default_model`
- `enabled`
- `created_at`
- `updated_at`

## 参考示例

- 见 `/Users/hanelalo/develop/dragon-li/docs/07-api-profiles-example.md`

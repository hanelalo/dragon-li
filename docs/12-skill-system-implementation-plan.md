# 12. Skill System Implementation Plan (基于 AgentSkills 规范)

## 1. 架构定位与设计原则
在 `dragon-li` 系统中，**Skill（技能）** 被定义为“特定领域的标准作业程序（SOP）和专家工作流”，而 **MCP** 被定义为“底层通用工具（I/O 能力）”。两者在概念上属于 `Capability` 模型，但在实现和执行上**完全解耦**。

设计原则严格遵循 Anthropic 官方的 [AgentSkills 规范 (agentskills.io)](https://agentskills.io/specification)：
- **目录即技能**：一个 Skill 必须是一个独立的文件夹。
- **配置与指令分离**：入口文件必须是 `SKILL.md`，包含 YAML Frontmatter（元数据）和 Markdown Body（SOP 指令）。
- **渐进式披露 (Progressive Disclosure)**：
  1. 闲聊模式仅注入 Metadata (name, description)；
  2. 触发后注入完整的 Instructions (Markdown Body)；
  3. 执行时按需加载 Resources (scripts, references)。
- **工具隔离与按需授权**：Skill 拥有独立的执行沙箱，仅挂载被允许的 MCP 工具（`allowed-tools`）或其专属的本地脚本工具。

## 2. 目录与文件规范
用户将 Skill 放置在 `~/.dragon-li/skills/` 目录下。

```text
~/.dragon-li/skills/
├── web-dev/                              # 文件夹名称即为 skill name
│   ├── SKILL.md                          # 必须：包含 YAML 元数据与 Markdown SOP
│   ├── scripts/                          # 可选：专属的本地执行脚本
│   │   └── generate_tree.py
│   ├── references/                       # 可选：参考文档
│   └── assets/                           # 可选：静态资源
```

**SKILL.md 示例：**
```markdown
---
name: web-dev
description: 前端开发专家，从零构建或重构高质量的 Web 项目。当用户提到“写网页”、“前端开发”时使用。
allowed-tools: mcp_local_fs__read_file mcp_local_fs__write_file
---
# Web 前端开发 SOP
你是一个拥有 10 年经验的资深前端架构师...
<instructions>
1. 需求分析：...
2. 架构设计：...
</instructions>
```

## 3. 触发机制设计
### 3.1 显式触发 (Explicit Trigger)
- **前端交互**：用户在输入框输入 `@` 时，弹出一个包含所有已启用 Skill 的下拉菜单。
- **UI 呈现**：用户选中某个 Skill 后，在输入框中插入一个不可编辑的整体原子节点（如 `<span class="mention">@web-dev</span>`）。当用户按 Backspace 键时，该节点作为一个整体被删除，而不是逐字删除。
- **数据传输**：前端解析输入框内容，提取出显式触发的 Skill ID，并将其随用户的原始问题一并发送给后端（例如 `{"text": "帮我写个网页", "explicit_skill": "web-dev"}`）。
- **后端处理**：后端直接绕过意图识别，加载对应的 `SKILL.md` 替换 System Prompt。

### 3.2 隐式触发 (Implicit Trigger / LLM Routing)
- **拒绝正则匹配**：不使用正则表达式进行意图匹配，而是将选择权交给大模型。
- **动态 Schema 注入**：后端在普通闲聊模式下，动态构建一个 `delegate_to_skill` 工具，其 Schema 中的 `skill_name` 必须是一个枚举（`enum`），包含了所有当前已启用的 Skill 名称，并且在工具描述中包含所有可用 Skill 的描述信息。
- **任务委派**：大模型判断需要调用某专家时，输出 `delegate_to_skill` Tool Call。大模型不仅仅提供 `skill_name`，还负责提供 `task_context`（大模型为该专家提炼的明确任务说明和上下文，而不是直接带着用户的原始问题）。
- **静默拦截**：系统在 `llm_provider.py` 中静默拦截此工具调用（不向用户展示该 Tool Call），加载目标 Skill 的 Markdown Body 替换 System Prompt，并将 `task_context` 作为新的输入，重新发起大模型请求。

## 4. 工具与上下文隔离
- **独立的 Skill Manager**：新增全局单例 `SkillManager`（与 `McpClientManager` 完全解耦且平级）。`SkillManager` 负责扫描解析本地文件、构建 `delegate_to_skill` Schema、以及管理专属工具。
- **按需过滤 MCP 工具**：当系统进入某个 Skill 的上下文时，**不再全量挂载**所有的 MCP 工具。而是根据该 Skill 的 `SKILL.md` 中的 `allowed-tools` 字段，通过 `SkillManager` 去 `McpClientManager` 中挑选指定的全局工具。
- **挂载专属本地工具**：`SkillManager` 将 `scripts/` 目录下的可执行脚本解析为本地专属工具 Schema 并挂载给大模型。在执行时，严格限制在沙箱目录（该 Skill 的文件夹）内运行子进程。

## 5. 实施路线图 (OpenSpec Changes)
本方案将被拆解为以下 5 个小步快跑的 Change，按照 OpenSpec 规范逐步交付：
1. `change-18-skill-discovery-and-db`: 目录扫描、解析 `SKILL.md` (YAML Frontmatter) 并同步至 SQLite 数据库的 `capabilities` 表。
2. `change-19-skill-management-ui`: 桌面端展示 Skill 列表与启用/禁用开关页面。
3. `change-20-composer-explicit-trigger`: 前端输入框重构为支持富文本的编辑器（如 `tiptap`），实现 `@` 唤出和原子节点提及，并在请求时携带 `explicit_skill` 字段。
4. `change-21-skill-implicit-trigger`: Python 端在 `SkillManager` 中动态构建带 Enum 的 `delegate_to_skill` 路由工具，并在 `llm_provider.py` 中实现静默拦截与 Context 替换。
5. `change-22-skill-tool-isolation`: 完善 `SkillManager`，在 Skill 执行上下文中根据 `allowed-tools` 按需挂载 MCP 工具，并解析执行专属 `scripts/`。
# 兼容政策

首个 CLI package 版本是 `0.1.0`，Git tag 为 `derivon-cli-v0.1.0`，用 package 前缀避免与 workspace 现有 tag 冲突。

1.0 之前，同一 minor 内的 patch 保持契约兼容；新的 `0.x` minor 可以通过 release notes 引入不兼容变化。1.0 之后，不兼容的命令、成功输出、error code 或 apply operation 变化需要新的 CLI major。兼容版本可以新增命令、可选 flag 和 error code，消费者必须处理未知 code。自动化应固定兼容版本线，例如 `0.1.x`。

`derivon.graph/v1` 发布后冻结，即使 CLI 仍低于 1.0，不兼容 graph 结构也必须使用新 schema。缺少 schema 永远表示 graph/v1。CLI 不隐式升级或降级 graph。

GitHub Pages 将 main 发布到 `https://docs.derivon.net/cli/dev/`。`derivon-cli-v0.1.0` 等 tag 将最新 `0.1.x` 手册发布到 `/cli/v0.1/`；1.0 后按 major 发布 `/cli/v1/`。`/cli/` 指向最新 release，站点根当前指向 `/cli/`；英文为规范，中文位于各版本的 `/zh/`。

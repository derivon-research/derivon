# Derivon CLI

Derivon CLI 是面向 Agent、脚本和 Unix 管道的无状态 JSON 处理器，用于非负加权有向 B-超图。

英文手册是规范文本；中文手册是同步维护的翻译。两者冲突时，以英文契约为准。

CLI 只接收 graph，不接收完整的 Derivon authoring 文档：

```bash
jq '.graph' workspace.json | derivon validate
```

每次调用只处理一个完整 graph。CLI 不保存会话状态，也不原地修改输入文件。

## 数学范围

一条 Derivon 超边包含零个或多个尾点、恰好一个头点和一个非负权重：

```text
h = (tails, head, weight)
```

所有尾点同时满足后才能推导头点。指向同一头点的多条超边表示备选推导。空尾、平行超边和环都合法。Derivon 不是通用图 CLI。

成功结果只向 stdout 写 JSON；错误以结构化 JSON 写入 stderr。mutation 输出完整 graph，读取和查询命令只输出结果。默认输出紧凑 JSON，`--pretty` 使用两空格缩进，成功 JSON 以一个换行结束。

`--help`、命令级 `--help` 和 `--version` 是仅有的纯文本 stdout 例外，并且不会读取 graph。

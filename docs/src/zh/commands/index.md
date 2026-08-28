# 命令

```text
validate
point list|get|add|remove|rename|data get|data set|data remove
hyperedge list|get|add|remove|rename|set tails|set head|set weight|data get|data set|data remove
query closure|route|diagnose
subgraph route|reachable|induced
apply
```

全局参数为 `--input`、`--pretty`、`--max-input-bytes`、`--max-value-bytes`、`--help` 和 `--version`。graph 默认来自 stdin；存在 `--input` 时不检查 stdin。字节限制必须为大于零的十进制数。业务参数只提供稳定长名称。

mutation 输出完整 graph；读取直接输出实体或 JSON 值；`validate` 透传合法 graph；query 不修改输入。成功 JSON 默认紧凑，`--pretty` 使用两空格。

`--data`/`--value` 接收 JSON 文本，并分别与 file 形式互斥。普通字符串需要 JSON 引号。pointer 为空或以 `/` 开头。
